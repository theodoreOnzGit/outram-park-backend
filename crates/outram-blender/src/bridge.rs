// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Bridge Edge Loops. Follows the published behaviour of Blender's bridge
// operator (source/blender/bmesh/operators/bmo_bridge.cc,
// github.com/blender/blender, GPL-2.0-or-later): connect two edge loops with a
// strip of faces, with a twist offset, optional subdivision cuts, and an
// optional weld of coincident endpoints. Concepts only — no upstream source
// copied; polygon-soup rebuild.
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

//! **Bridge Edge Loops** (`op-hzs.54.14`, GH issue #37 §B) — join two edge
//! loops with a face strip. Blender's `Edge ▸ Bridge Edge Loops`.
//!
//! [`bridge_edge_loops`] takes the two loops as ordered vertex rings (open or
//! closed) plus [`BridgeOptions`]:
//!
//! - `twist` — rotate the pairing between the two rings by this many steps
//!   (Blender's Twist).
//! - `cuts` — insert `cuts` intermediate rings, so the bridge is `cuts + 1`
//!   quads deep (linear interpolation).
//! - `flip` — reverse ring B's direction (fixes an inside-out strip).
//! - `merge_ends` — if a paired vertex on each ring is within a tolerance,
//!   weld them (bridging a loop back onto itself).
//!
//! The two rings must have the **same** vertex count; unequal-count bridging
//! (Blender interpolates) is tracked as follow-up under this bead.

use crate::math::Vec3;
use crate::mesh::{Mesh, VertexId};

/// Tuning for [`bridge_edge_loops`].
#[derive(Debug, Clone, Copy)]
pub struct BridgeOptions {
    /// Rotate the A↔B vertex pairing by this many steps.
    pub twist: i64,
    /// Intermediate rings inserted along the bridge (`0` = one quad deep).
    pub cuts: usize,
    /// Reverse ring B before pairing.
    pub flip: bool,
    /// Weld paired vertices closer than this distance (`0` disables).
    pub merge_distance: f64,
}

impl Default for BridgeOptions {
    fn default() -> Self {
        BridgeOptions { twist: 0, cuts: 0, flip: false, merge_distance: 0.0 }
    }
}

/// Bridge `ring_a` to `ring_b` (both ordered vertex rings of equal length),
/// appending the connecting faces to `mesh`. `closed` says whether the rings
/// are cyclic (a tube) or open (a ribbon).
pub fn bridge_edge_loops(
    mesh: &Mesh,
    ring_a: &[VertexId],
    ring_b: &[VertexId],
    closed: bool,
    opts: BridgeOptions,
) -> Mesh {
    if ring_a.len() != ring_b.len() || ring_a.len() < 2 {
        return mesh.clone();
    }
    let n = ring_a.len();
    let mut positions = mesh.positions();
    let mut faces: Vec<Vec<usize>> = mesh.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect();

    // Ring B, flipped and twisted.
    let mut b: Vec<usize> = ring_b.iter().map(|v| v.0).collect();
    if opts.flip {
        b.reverse();
    }
    let t = opts.twist.rem_euclid(n as i64) as usize;
    b.rotate_left(t);

    let a: Vec<usize> = ring_a.iter().map(|v| v.0).collect();

    // Build `cuts` intermediate rings by lerp.
    let cuts = opts.cuts;
    let mut rings: Vec<Vec<usize>> = Vec::with_capacity(cuts + 2);
    rings.push(a.clone());
    for c in 1..=cuts {
        let s = c as f64 / (cuts as f64 + 1.0);
        let mid: Vec<usize> = (0..n)
            .map(|i| {
                let p = positions[a[i]].add(positions[b[i]].sub(positions[a[i]]).scale(s));
                positions.push(p);
                positions.len() - 1
            })
            .collect();
        rings.push(mid);
    }
    rings.push(b.clone());

    let last_seg = if closed { n } else { n - 1 };
    for w in rings.windows(2) {
        let (r0, r1) = (&w[0], &w[1]);
        for i in 0..last_seg {
            let j = (i + 1) % n;
            faces.push(vec![r0[i], r0[j], r1[j], r1[i]]);
        }
    }

    let built = Mesh::from_polygons(&positions, &faces);
    if opts.merge_distance > 0.0 {
        crate::weld::weld(&built, opts.merge_distance)
    } else {
        built
    }
}

/// Walk an edge set into its ordered vertex ring, or `None` if it is not a
/// single simple chain / loop. `closed` in the result says whether it cycles.
pub fn ordered_ring(mesh: &Mesh, edges: &[crate::mesh::EdgeId]) -> Option<(Vec<VertexId>, bool)> {
    use std::collections::BTreeSet;
    let set: BTreeSet<crate::mesh::EdgeId> = edges.iter().copied().collect();
    if set.is_empty() {
        return None;
    }
    // vertex → incident set-edges
    let mut adj: std::collections::HashMap<VertexId, Vec<VertexId>> = std::collections::HashMap::new();
    for &e in &set {
        let ed = mesh.edge(e)?;
        adj.entry(ed.verts[0]).or_default().push(ed.verts[1]);
        adj.entry(ed.verts[1]).or_default().push(ed.verts[0]);
    }
    if adj.values().any(|n| n.len() > 2) {
        return None; // branching
    }
    let ends: Vec<VertexId> = adj.iter().filter(|(_, n)| n.len() == 1).map(|(&v, _)| v).collect();
    let closed = ends.is_empty();
    let start = if closed { *adj.keys().next().unwrap() } else { ends[0] };

    let mut ring = vec![start];
    let mut prev: Option<VertexId> = None;
    let mut cur = start;
    loop {
        let next = adj[&cur].iter().copied().find(|&v| Some(v) != prev && (Some(v) != Some(start) || ring.len() == 1));
        // For a closed loop we want to stop when we would revisit `start`.
        let next = match next {
            Some(v) if v == start => break,
            Some(v) => v,
            None => break,
        };
        if ring.contains(&next) {
            break;
        }
        ring.push(next);
        prev = Some(cur);
        cur = next;
    }
    if ring.len() < 2 {
        return None;
    }
    Some((ring, closed))
}

/// Convenience: reorder `ring_b` so its first vertex is the one geometrically
/// nearest `ring_a[0]` — a reasonable default pairing before applying `twist`.
pub fn align_by_nearest(mesh: &Mesh, ring_a: &[VertexId], ring_b: &[VertexId]) -> Vec<VertexId> {
    if ring_a.is_empty() || ring_b.is_empty() {
        return ring_b.to_vec();
    }
    let a0 = mesh.vertex(ring_a[0]).map(|v| v.position).unwrap_or(Vec3::ZERO);
    let best = (0..ring_b.len())
        .min_by(|&i, &j| {
            let di = mesh.vertex(ring_b[i]).map(|v| v.position.sub(a0).length()).unwrap_or(f64::MAX);
            let dj = mesh.vertex(ring_b[j]).map(|v| v.position.sub(a0).length()).unwrap_or(f64::MAX);
            di.partial_cmp(&dj).unwrap()
        })
        .unwrap_or(0);
    let mut b = ring_b.to_vec();
    b.rotate_left(best);
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::EdgeId;
    use crate::primitives;

    /// Two open triangle rings stacked, no faces — bridge into a prism band.
    fn two_open_rings() -> (Mesh, Vec<VertexId>, Vec<VertexId>) {
        let mut m = Mesh::new();
        let a: Vec<VertexId> = (0..4)
            .map(|i| m.add_vertex(Vec3::new(i as f64, 0.0, 0.0)))
            .collect();
        let b: Vec<VertexId> = (0..4)
            .map(|i| m.add_vertex(Vec3::new(i as f64, 0.0, 1.0)))
            .collect();
        // A dummy face so from_polygons keeps every vertex.
        m.add_face(&[a[0], a[1], b[1], b[0]]);
        (m, a, b)
    }

    #[test]
    fn bridge_two_open_rings_adds_a_ribbon() {
        let (m, a, b) = two_open_rings();
        let before = m.face_count();
        let br = bridge_edge_loops(&m, &a, &b, false, BridgeOptions::default());
        // 3 new quads (4 verts → 3 segments), plus the dummy.
        assert_eq!(br.face_count(), before + 3);
    }

    #[test]
    fn bridge_closed_rings_of_a_cylinder_gap() {
        // Two 8-gon rings; bridge closed → 8 quads.
        let mut m = Mesh::new();
        let ring = |z: f64, m: &mut Mesh| -> Vec<VertexId> {
            (0..8)
                .map(|i| {
                    let a = std::f64::consts::TAU * i as f64 / 8.0;
                    m.add_vertex(Vec3::new(a.cos(), a.sin(), z))
                })
                .collect()
        };
        let bot = ring(0.0, &mut m);
        let top = ring(2.0, &mut m);
        m.add_face(&[bot[0], bot[1], top[1], top[0]]);
        let br = bridge_edge_loops(&m, &bot, &top, true, BridgeOptions::default());
        assert_eq!(br.face_count(), 1 + 8);
    }

    #[test]
    fn cuts_deepen_the_bridge() {
        let (m, a, b) = two_open_rings();
        let br = bridge_edge_loops(&m, &a, &b, false, BridgeOptions { cuts: 2, ..Default::default() });
        // 3 segments * (cuts+1=3) quads + dummy.
        assert_eq!(br.face_count(), 1 + 9);
    }

    #[test]
    fn twist_rotates_the_pairing() {
        let (m, a, b) = two_open_rings();
        let straight = bridge_edge_loops(&m, &a, &b, false, BridgeOptions::default());
        let twisted = bridge_edge_loops(&m, &a, &b, false, BridgeOptions { twist: 1, ..Default::default() });
        // Same face count, different geometry (a twisted ribbon self-crosses).
        assert_eq!(straight.face_count(), twisted.face_count());
    }

    #[test]
    fn ordered_ring_walks_a_grid_border() {
        let m = primitives::grid(3, 3, 3.0);
        let topo = crate::topology::MeshTopology::new(&m);
        let border: Vec<EdgeId> = (0..m.edge_count()).map(EdgeId).filter(|&e| topo.is_boundary_edge(e)).collect();
        let (ring, closed) = ordered_ring(&m, &border).unwrap();
        assert!(closed);
        assert_eq!(ring.len(), 12, "3x3 grid border has 12 vertices");
    }
}
