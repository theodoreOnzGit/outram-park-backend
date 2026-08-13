// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Boundary-loop extraction plus centroid triangle-fan capping. No named published
// algorithm — written from first principles; no upstream source was copied.
// Blender analogue (architecture only): the bmo_holes_fill operator.
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

//! Fill holes — cap the open boundary loops of a surface so it becomes
//! watertight.
//!
//! This is the pure-Rust analogue of Blender's **Fill Holes**
//! (`bmesh` `bmo_holes_fill`): every boundary loop (a closed chain of edges
//! each incident to only one face) is capped with a triangle fan to the loop's
//! centroid, closing the surface. Together with [`crate::weld`] it is the
//! mesh-repair pair the export bridges rely on — weld stitches near-coincident
//! seams, fill-holes closes genuine gaps — so that an open authored surface can
//! become the closed geometry the [`crate::export`] Monte-Carlo CSG bridge
//! needs.
//!
//! # How a hole is found and capped
//!
//! A **boundary edge** is a directed half-edge `a → b` (from a face's winding)
//! whose reverse `b → a` appears in no face — i.e. the edge borders exactly one
//! face. Chaining boundary half-edges tail-to-head recovers each boundary
//! **loop**. Each loop is capped by adding one **centroid** vertex at the
//! average of the loop's positions and one triangle per boundary edge.
//!
//! ## Winding
//!
//! For the cap to be consistent with the existing surface, two adjacent faces
//! must traverse their shared edge in **opposite** directions. A boundary edge
//! `a → b` belongs to an existing face that traverses it `a → b`, so its cap
//! triangle must traverse it `b → a`; the emitted triangle is therefore
//! `[b, a, centroid]`. This keeps every face's outward normal consistent
//! without any explicit normal computation.
//!
//! # Scope
//!
//! The centroid fan is always topologically valid and makes the surface
//! watertight, but for a strongly non-planar or non-convex hole the fan is a
//! *valid* cap, not a *minimal-area* or *beauty* triangulation — an honest
//! limitation, documented rather than hidden. A loop with fewer than three
//! edges (degenerate) is left uncapped. No `faer`, no external dependency;
//! Android-safe.

use crate::math::Vec3;
use crate::mesh::Mesh;
use std::collections::{HashMap, HashSet};

/// Cap every open boundary loop of `mesh` with a centroid triangle fan,
/// returning the watertight mesh.
///
/// A mesh that is already closed (no boundary edges) is returned unchanged (a
/// no-op rebuild). Each hole adds one centroid vertex and one triangle per
/// boundary edge; the winding of every cap triangle is chosen to stay
/// consistent with the surrounding surface.
///
/// This is infallible: the result is always a valid mesh. Degenerate boundary
/// loops (fewer than three edges) are left uncapped.
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, fill_holes::fill_holes};
///
/// // A closed cube has no holes, so filling is a no-op: still V=8, F=6, chi=2.
/// let cube = primitives::cube(2.0);
/// let filled = fill_holes(&cube);
/// assert_eq!(filled.vertex_count(), 8);
/// assert_eq!(filled.face_count(), 6);
/// assert_eq!(filled.euler_characteristic(), 2);
/// ```
pub fn fill_holes(mesh: &Mesh) -> Mesh {
    let positions = mesh.positions();
    let polygons = mesh.polygons();

    // Collect every directed half-edge present in the surface.
    let mut directed: HashSet<(usize, usize)> = HashSet::new();
    for poly in &polygons {
        let k = poly.len();
        for i in 0..k {
            let a = poly[i].0;
            let b = poly[(i + 1) % k].0;
            directed.insert((a, b));
        }
    }

    // A boundary half-edge is one whose reverse is absent. Index them by tail so
    // we can chain each loop head-to-tail.
    let mut next_of: HashMap<usize, usize> = HashMap::new();
    for &(a, b) in &directed {
        if !directed.contains(&(b, a)) {
            // On a manifold boundary each tail has a unique outgoing boundary
            // edge; if not (non-manifold), we keep the first seen — best effort.
            next_of.entry(a).or_insert(b);
        }
    }

    // Start from the original geometry and append caps.
    let mut new_positions = positions.clone();
    let mut new_faces: Vec<Vec<usize>> = polygons
        .iter()
        .map(|p| p.iter().map(|v| v.0).collect())
        .collect();

    // Trace each boundary loop exactly once.
    let mut visited: HashSet<usize> = HashSet::new();
    for &start in next_of.keys() {
        if visited.contains(&start) {
            continue;
        }
        // Walk the chain start -> next -> ... back to start, collecting the
        // ordered boundary edges of this loop.
        let mut loop_edges: Vec<(usize, usize)> = Vec::new();
        let mut cur = start;
        loop {
            if !visited.insert(cur) {
                break; // closed the loop (or hit an already-traced vertex)
            }
            let Some(&nxt) = next_of.get(&cur) else {
                break; // broken chain (non-manifold); abandon this partial loop
            };
            loop_edges.push((cur, nxt));
            cur = nxt;
            if cur == start {
                break;
            }
        }
        // Need at least a triangle's worth of boundary to cap.
        if loop_edges.len() < 3 {
            continue;
        }
        // Centroid of the loop's tail vertices.
        let mut acc = Vec3::ZERO;
        for &(a, _) in &loop_edges {
            acc = acc.add(new_positions[a]);
        }
        let centroid = acc.scale(1.0 / loop_edges.len() as f64);
        let c = new_positions.len();
        new_positions.push(centroid);
        // Cap triangle per boundary edge, reversed for consistent winding.
        for &(a, b) in &loop_edges {
            new_faces.push(vec![b, a, c]);
        }
    }

    Mesh::from_polygons(&new_positions, &new_faces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// Remove face `drop_idx` from a mesh, returning the open polygon soup.
    fn remove_face(mesh: &Mesh, drop_idx: usize) -> Mesh {
        let ps = mesh.positions();
        let faces: Vec<Vec<usize>> = mesh
            .polygons()
            .into_iter()
            .enumerate()
            .filter(|(i, _)| *i != drop_idx)
            .map(|(_, poly)| poly.into_iter().map(|v| v.0).collect())
            .collect();
        Mesh::from_polygons(&ps, &faces)
    }

    /// True when every undirected edge borders exactly two faces — i.e. each
    /// directed half-edge is unique and its reverse is present (a watertight,
    /// consistently-wound closed surface).
    fn is_watertight_consistent(mesh: &Mesh) -> bool {
        let mut directed: HashSet<(usize, usize)> = HashSet::new();
        for poly in mesh.polygons() {
            let k = poly.len();
            for i in 0..k {
                let a = poly[i].0;
                let b = poly[(i + 1) % k].0;
                if !directed.insert((a, b)) {
                    return false; // duplicated half-edge -> inconsistent
                }
            }
        }
        directed.iter().all(|&(a, b)| directed.contains(&(b, a)))
    }

    /// V&V — headline. Methodology: take `cube(2.0)` (V8 E12 F6 χ2), drop one
    /// quad face to open a 4-edge square hole, then fill. Pass criterion: the
    /// surface is closed again and watertight. Result: V 8→9 (one centroid),
    /// F 5→9 (4 cap triangles), E 12→16, χ = 9 − 16 + 9 = 2, and every edge
    /// borders exactly two consistently-wound faces.
    #[test]
    fn open_cube_face_is_capped_watertight() {
        let open = remove_face(&primitives::cube(2.0), 0);
        assert_eq!(open.face_count(), 5, "one face removed");
        assert!(
            !is_watertight_consistent(&open),
            "open mesh is not watertight"
        );

        let filled = fill_holes(&open);
        assert_eq!(filled.vertex_count(), 9, "8 corners + 1 hole centroid");
        assert_eq!(filled.face_count(), 9, "5 quads + 4 cap triangles");
        assert_eq!(filled.euler_characteristic(), 2, "closed genus-0 surface");
        assert!(
            is_watertight_consistent(&filled),
            "cap winding is consistent"
        );
    }

    /// V&V — an already-closed mesh is a no-op. Filling a pristine cube changes
    /// nothing (no boundary edges to trace).
    #[test]
    fn closed_mesh_is_unchanged() {
        let cube = primitives::cube(2.0);
        let filled = fill_holes(&cube);
        assert_eq!(filled.vertex_count(), 8);
        assert_eq!(filled.face_count(), 6);
        assert_eq!(filled.edge_count(), 12);
        assert_eq!(filled.euler_characteristic(), 2);
        assert!(is_watertight_consistent(&filled));
    }

    /// V&V — two separate holes are both capped. Drop two opposite faces of the
    /// cube (a tube open at both ends); fill closes both independent loops.
    #[test]
    fn two_holes_are_both_capped() {
        // Faces 0 and 1 of the cube are the +X / −X caps (opposite) by
        // construction; dropping both leaves a 4-sided tube with two square
        // holes.
        let ps = primitives::cube(2.0).positions();
        let faces: Vec<Vec<usize>> = primitives::cube(2.0)
            .polygons()
            .into_iter()
            .enumerate()
            .filter(|(i, _)| *i != 0 && *i != 1)
            .map(|(_, poly)| poly.into_iter().map(|v| v.0).collect())
            .collect();
        let tube = Mesh::from_polygons(&ps, &faces);
        assert_eq!(tube.face_count(), 4);
        assert!(!is_watertight_consistent(&tube));

        let filled = fill_holes(&tube);
        // 8 corners + 2 centroids; 4 side quads + 2×4 cap triangles.
        assert_eq!(filled.vertex_count(), 10);
        assert_eq!(filled.face_count(), 12);
        assert_eq!(filled.euler_characteristic(), 2);
        assert!(is_watertight_consistent(&filled));
    }

    /// V&V — the `MeshOp::FillHoles` dispatch matches the free function.
    #[test]
    fn meshop_fill_holes_matches_free_function() {
        use crate::ops::MeshOp;
        let open = remove_face(&primitives::cube(2.0), 3);
        let direct = fill_holes(&open);
        let via_op = MeshOp::FillHoles.apply(open).expect("fill is infallible");
        assert_eq!(via_op.vertex_count(), direct.vertex_count());
        assert_eq!(via_op.face_count(), direct.face_count());
        assert_eq!(via_op.euler_characteristic(), direct.euler_characteristic());
    }
}
