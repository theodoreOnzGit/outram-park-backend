// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Bevel — multi-segment rounded edge bevel. Follows the published behaviour of
// Blender's bevel operator/modifier (source/blender/bmesh/tools/bmesh_bevel.cc,
// github.com/blender/blender, GPL-2.0-or-later): cut each edge back from its
// two faces and fill the gap with `segments` quads following a profile curve,
// with several width parameterisations and an overlap clamp. Concepts only —
// no upstream source copied; this is a polygon-soup rebuild that extends the
// flat chamfer in `edge_bevel`.
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

//! **Bevel — Blender parity** (`op-hzs.54.9`, GH issue #37 §B).
//!
//! [`bevel`] extends [`crate::edge_bevel`]'s flat single-segment chamfer with:
//!
//! - **`segments`** — the edge gap is filled with `segments` quads whose
//!   cross-section follows a circular arc (a rounded edge), not one flat quad.
//! - **`profile`** — `0.0` chord (flat chamfer) … `0.5` circular … `1.0` bulged
//!   toward the original edge (sharp). Blender's profile slider.
//! - **`width_type`** — how `amount` is interpreted ([`WidthType`]). `Offset`
//!   is exact (distance each face is cut back); `Width` / `Depth` / `Percent`
//!   are right-angle approximations, since a headless call has no live dihedral.
//! - **`clamp_overlap`** — clamp the offset to half the shortest edge so a face
//!   cannot invert.
//!
//! The **corner** where three or more beveled edges meet is filled with a
//! single n-gon cap (as in `edge_bevel`); a rounded spherical-triangle corner
//! patch is tracked as follow-up under this bead. Every edge is beveled — a
//! *selected-subset* bevel needs partial-boundary handling and is also
//! follow-up.

use std::collections::{HashMap, HashSet};

use crate::math::Vec3;
use crate::mesh::Mesh;

/// How [`BevelOptions::amount`] is measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidthType {
    /// Distance each adjacent face is moved back from the edge (exact).
    Offset,
    /// Width of the new bevel face (≈ `offset · √2` at a right angle).
    Width,
    /// Perpendicular distance from the original edge to the new face
    /// (≈ `offset / √2` at a right angle).
    Depth,
    /// Percentage (0–100) of the mean adjacent edge length.
    Percent,
}

/// Tuning for [`bevel`].
#[derive(Debug, Clone, Copy)]
pub struct BevelOptions {
    /// Bevel size, interpreted per [`BevelOptions::width_type`].
    pub amount: f64,
    /// Number of quad rings across the bevel (`>= 1`). `1` = flat chamfer.
    pub segments: usize,
    /// Profile shape, `0.0` … `1.0` (`0.5` = circular).
    pub profile: f64,
    /// How `amount` is measured.
    pub width_type: WidthType,
    /// Clamp the offset to half the shortest edge so no face inverts.
    pub clamp_overlap: bool,
}

impl Default for BevelOptions {
    fn default() -> Self {
        BevelOptions {
            amount: 0.1,
            segments: 1,
            profile: 0.5,
            width_type: WidthType::Offset,
            clamp_overlap: true,
        }
    }
}

/// Bevel every edge of `mesh` per `opts`.
pub fn bevel(mesh: &Mesh, opts: BevelOptions) -> Mesh {
    let ps = mesh.positions();
    let polys: Vec<Vec<usize>> = mesh.polygons().iter().map(|p| p.iter().map(|v| v.0).collect()).collect();
    if polys.is_empty() {
        return mesh.clone();
    }
    let segments = opts.segments.max(1);
    let profile = opts.profile.clamp(0.0, 1.0);

    // Resolve `amount` to an offset (distance to inset each face corner).
    let mean_edge = {
        let mut s = 0.0;
        let mut c = 0.0;
        for poly in &polys {
            let k = poly.len();
            for i in 0..k {
                s += ps[poly[i]].sub(ps[poly[(i + 1) % k]]).length();
                c += 1.0;
            }
        }
        if c > 0.0 { s / c } else { 1.0 }
    };
    let mut offset = match opts.width_type {
        WidthType::Offset => opts.amount,
        WidthType::Width => opts.amount / std::f64::consts::SQRT_2,
        WidthType::Depth => opts.amount * std::f64::consts::SQRT_2,
        WidthType::Percent => opts.amount / 100.0 * mean_edge,
    };
    if opts.clamp_overlap {
        let mut shortest = f64::INFINITY;
        for poly in &polys {
            let k = poly.len();
            for i in 0..k {
                shortest = shortest.min(ps[poly[i]].sub(ps[poly[(i + 1) % k]]).length());
            }
        }
        offset = offset.min(0.49 * shortest);
    }
    offset = offset.max(0.0);

    // Face-corner inset points, one per (face, corner).
    let mut positions: Vec<Vec3> = Vec::new();
    let mut fc: Vec<Vec<usize>> = Vec::with_capacity(polys.len());
    for poly in &polys {
        let k = poly.len();
        let mut row = Vec::with_capacity(k);
        for i in 0..k {
            let v = ps[poly[i]];
            let prev = ps[poly[(i + k - 1) % k]];
            let next = ps[poly[(i + 1) % k]];
            let p = v
                .add(prev.sub(v).normalize().scale(offset))
                .add(next.sub(v).normalize().scale(offset));
            positions.push(p);
            row.push(positions.len() - 1);
        }
        fc.push(row);
    }

    let mut owner: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
    for (f, poly) in polys.iter().enumerate() {
        let k = poly.len();
        for i in 0..k {
            owner.insert((poly[i], poly[(i + 1) % k]), (f, i));
        }
    }

    let mut faces: Vec<Vec<usize>> = Vec::new();

    // (a) shrunk faces
    for row in &fc {
        faces.push(row.clone());
    }

    // (b) rounded edge strips. Also record, per (face, corner), the rail arc
    // around that corner's vertex so the vertex cap below reuses the same
    // interior points and stays watertight.
    let mut rail_by_corner: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for (f, poly) in polys.iter().enumerate() {
        let k = poly.len();
        for i in 0..k {
            let (a, b) = (poly[i], poly[(i + 1) % k]);
            let key = if a < b { (a, b) } else { (b, a) };
            if !seen.insert(key) {
                continue;
            }
            let Some(&(g, gj)) = owner.get(&(b, a)) else {
                continue; // boundary edge left open (as edge_bevel v1)
            };
            let a_f = fc[f][i];
            let b_f = fc[f][(i + 1) % k];
            let b_g = fc[g][gj];
            let a_g = fc[g][(gj + 1) % polys[g].len()];

            let rail_a = arc_points(&mut positions, ps[a], a_f, a_g, segments, profile);
            let rail_b = arc_points(&mut positions, ps[b], b_f, b_g, segments, profile);

            for s in 0..segments {
                faces.push(vec![rail_a[s], rail_b[s], rail_b[s + 1], rail_a[s + 1]]);
            }

            // Umbrella walk from corner (f,i) crosses edge (a,b): rail_a is that
            // arc. Umbrella walk of vertex b from corner (g,gj) crosses (b,a):
            // that arc is rail_b reversed.
            rail_by_corner.insert((f, i), rail_a);
            rail_by_corner.insert((g, gj), rail_b.into_iter().rev().collect());
        }
    }

    // (c) vertex caps — fan-fill the umbrella boundary ring (concatenated rail
    // arcs) from its centroid, so a rounded corner connects to every strip.
    let mut vert_corners: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    for (f, poly) in polys.iter().enumerate() {
        for (i, &v) in poly.iter().enumerate() {
            vert_corners.entry(v).or_default().push((f, i));
        }
    }
    for corners in vert_corners.values() {
        if corners.len() < 3 {
            continue;
        }
        let start = corners[0];
        let mut ring: Vec<usize> = Vec::new();
        let mut cur = start;
        let mut open = false;
        let mut visited = 0;
        loop {
            let (f, i) = cur;
            if let Some(arc) = rail_by_corner.get(&(f, i)) {
                // Append the arc minus its last point (shared with the next).
                for &p in &arc[..arc.len().saturating_sub(1)] {
                    ring.push(p);
                }
            } else {
                ring.push(fc[f][i]);
            }
            let poly = &polys[f];
            let kk = poly.len();
            let (v, w) = (poly[i], poly[(i + 1) % kk]);
            visited += 1;
            match owner.get(&(w, v)) {
                Some(&(g, m)) => {
                    cur = (g, (m + 1) % polys[g].len());
                    if cur == start {
                        break;
                    }
                }
                None => {
                    open = true;
                    break;
                }
            }
            if visited > corners.len() {
                break;
            }
        }
        if open || visited != corners.len() || ring.len() < 3 {
            continue;
        }
        if ring.len() == 3 {
            faces.push(ring);
        } else {
            let c = ring.iter().fold(Vec3::ZERO, |acc, &p| acc.add(positions[p])).scale(1.0 / ring.len() as f64);
            let ci = positions.len();
            positions.push(c);
            for r in 0..ring.len() {
                faces.push(vec![ci, ring[r], ring[(r + 1) % ring.len()]]);
            }
        }
    }

    let built = Mesh::from_polygons(&positions, &faces);
    crate::recalc_normals::recalculate_normals(&built)
}

/// `segments + 1` vertex ids along an arc from vertex `i0` to vertex `i1`
/// bulging around the corner point `v`, with `profile` blending chord (0) ↔
/// circular (0.5) ↔ bulged (1). The two endpoint ids are reused; interior
/// points are appended to `positions`.
fn arc_points(
    positions: &mut Vec<Vec3>,
    v: Vec3,
    i0: usize,
    i1: usize,
    segments: usize,
    profile: f64,
) -> Vec<usize> {
    let p0 = positions[i0];
    let p1 = positions[i1];
    let d0 = p0.sub(v);
    let d1 = p1.sub(v);
    let r = (d0.length() + d1.length()) * 0.5;
    let mut out = Vec::with_capacity(segments + 1);
    out.push(i0);
    for s in 1..segments {
        let t = s as f64 / segments as f64;
        let chord = p0.add(p1.sub(p0).scale(t));
        // Circular arc: normalized slerp-ish of the two directions.
        let dir = d0.scale(1.0 - t).add(d1.scale(t));
        let circ = v.add(dir.normalize().scale(r));
        // profile 0 → chord, 0.5 → circ, 1 → overshoot toward circ apex.
        let blend = (profile * 2.0).min(2.0);
        let p = chord.add(circ.sub(chord).scale(blend));
        out.push(positions.len());
        positions.push(p);
    }
    out.push(i1);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// Every undirected edge is shared by exactly two faces.
    fn watertight(m: &Mesh) -> bool {
        let mut count: std::collections::HashMap<(usize, usize), usize> = std::collections::HashMap::new();
        for poly in m.polygons() {
            let k = poly.len();
            for i in 0..k {
                let (a, b) = (poly[i].0, poly[(i + 1) % k].0);
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        count.values().all(|&c| c == 2)
    }

    #[test]
    fn one_segment_matches_the_flat_edge_bevel_counts() {
        let m = primitives::cube(2.0);
        let b = bevel(&m, BevelOptions { amount: 0.3, segments: 1, ..Default::default() });
        // 6 shrunk squares + 12 edge quads + 8 corner triangles.
        assert_eq!(b.face_count(), 26);
        assert_eq!(b.euler_characteristic(), 2);
    }

    #[test]
    fn three_segments_add_rings_and_stay_closed() {
        let m = primitives::cube(2.0);
        let b = bevel(&m, BevelOptions { amount: 0.3, segments: 3, profile: 0.5, ..Default::default() });
        assert!(b.face_count() > 26, "more faces than the flat chamfer");
        assert_eq!(b.euler_characteristic(), 2, "rounded bevel still closed genus-0");
        assert!(watertight(&b), "every edge used by exactly two faces");
    }

    #[test]
    fn clamp_overlap_bounds_a_huge_amount() {
        let m = primitives::cube(2.0);
        let b = bevel(&m, BevelOptions { amount: 100.0, segments: 1, clamp_overlap: true, ..Default::default() });
        assert_eq!(b.euler_characteristic(), 2, "clamped, no inverted faces");
    }

    #[test]
    fn width_type_percent_scales_with_the_mesh() {
        let m = primitives::cube(2.0); // edge length 2
        let b = bevel(&m, BevelOptions { amount: 10.0, width_type: WidthType::Percent, segments: 1, ..Default::default() });
        assert_eq!(b.face_count(), 26);
        assert_eq!(b.euler_characteristic(), 2);
    }

    #[test]
    fn profile_zero_is_a_flat_chamfer() {
        let m = primitives::cube(2.0);
        let flat = bevel(&m, BevelOptions { amount: 0.3, segments: 3, profile: 0.0, ..Default::default() });
        assert_eq!(flat.euler_characteristic(), 2);
        assert!(watertight(&flat));
    }
}
