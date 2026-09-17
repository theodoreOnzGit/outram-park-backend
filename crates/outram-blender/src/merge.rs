// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Merge. Follows the published behaviour of Blender's merge operator
// (source/blender/editors/mesh/editmesh_tools.cc `MESH_OT_merge`,
// github.com/blender/blender, GPL-2.0-or-later): collapse a set of vertices to
// one point — their centre, a given point, the first / last of the set — or
// collapse selected edges; plus merge-by-distance over a subset. Concepts only
// — no upstream source copied; this is a vertex-remap + degenerate-face drop.
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

//! **Merge** (`op-hzs.54.11`, GH issue #37 §B) — collapse vertices together.
//!
//! - [`merge_vertices`] collapses a vertex set to one point chosen by
//!   [`MergeTarget`] (centre / a supplied point / the first / the last of the
//!   set). This is Blender's `M` menu.
//! - [`merge_edges`] collapses each edge in a set to its midpoint (Blender's
//!   *Collapse*), independently.
//! - [`merge_by_distance`] merges only vertices *within* a given set that are
//!   closer than a threshold — the subset form of
//!   [`crate::weld::weld`] / Blender's *Merge by Distance* and the operation
//!   an **Auto-Merge** editor toggle runs after each edit.

use std::collections::HashMap;

use crate::math::Vec3;
use crate::mesh::{EdgeId, Mesh, VertexId};

/// Where [`merge_vertices`] places the merged vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MergeTarget {
    /// The arithmetic mean of the merged vertices' positions.
    Center,
    /// A caller-supplied point (Blender's *At Cursor*).
    Point(Vec3),
    /// The position of the first vertex in the set (ascending id).
    First,
    /// The position of the last vertex in the set (ascending id).
    Last,
}

/// Collapse `verts` into a single vertex placed per `target`. Faces that become
/// degenerate (fewer than three distinct corners) are dropped. Returns the
/// rebuilt mesh.
pub fn merge_vertices(mesh: &Mesh, verts: &[VertexId], target: MergeTarget) -> Mesh {
    let mut set: Vec<usize> = verts
        .iter()
        .map(|v| v.0)
        .filter(|&v| v < mesh.vertex_count())
        .collect();
    set.sort_unstable();
    set.dedup();
    if set.len() < 2 {
        return mesh.clone();
    }
    let positions = mesh.positions();
    let pos = match target {
        MergeTarget::Center => set
            .iter()
            .fold(Vec3::ZERO, |acc, &i| acc.add(positions[i]))
            .scale(1.0 / set.len() as f64),
        MergeTarget::Point(p) => p,
        MergeTarget::First => positions[set[0]],
        MergeTarget::Last => positions[*set.last().unwrap()],
    };
    let keep = set[0];
    let remap: HashMap<usize, usize> = set.iter().skip(1).map(|&v| (v, keep)).collect();
    rebuild(mesh, &remap, &HashMap::from([(keep, pos)]))
}

/// Collapse each edge in `edges` to its midpoint, independently (Blender's
/// *Merge ▸ Collapse*). Chained edges collapse toward a shared vertex.
pub fn merge_edges(mesh: &Mesh, edges: &[EdgeId]) -> Mesh {
    let positions = mesh.positions();
    let mut remap: HashMap<usize, usize> = HashMap::new();
    let mut moved: HashMap<usize, Vec3> = HashMap::new();
    for &e in edges {
        let Some(ed) = mesh.edge(e) else { continue };
        let (a, b) = (ed.verts[0].0, ed.verts[1].0);
        let ra = resolve(&remap, a);
        let rb = resolve(&remap, b);
        if ra == rb {
            continue;
        }
        let keep = ra.min(rb);
        let gone = ra.max(rb);
        remap.insert(gone, keep);
        let mid = positions[a].add(positions[b]).scale(0.5);
        moved.insert(keep, mid);
    }
    if remap.is_empty() {
        return mesh.clone();
    }
    // Fold moved positions onto the final representative of each cluster.
    let final_moves: HashMap<usize, Vec3> = moved
        .into_iter()
        .map(|(v, p)| (resolve(&remap, v), p))
        .collect();
    rebuild(mesh, &remap, &final_moves)
}

/// Merge vertices *within* `verts` that lie within `threshold` of each other,
/// keeping the lowest id of each cluster. The subset form of
/// [`crate::weld::weld`].
pub fn merge_by_distance(mesh: &Mesh, verts: &[VertexId], threshold: f64) -> Mesh {
    let mut set: Vec<usize> = verts
        .iter()
        .map(|v| v.0)
        .filter(|&v| v < mesh.vertex_count())
        .collect();
    set.sort_unstable();
    set.dedup();
    let positions = mesh.positions();
    let t2 = threshold.max(0.0).powi(2);

    let mut remap: HashMap<usize, usize> = HashMap::new();
    for i in 0..set.len() {
        let a = set[i];
        if remap.contains_key(&a) {
            continue;
        }
        for &b in &set[i + 1..] {
            if remap.contains_key(&b) {
                continue;
            }
            if positions[a]
                .sub(positions[b])
                .dot(positions[a].sub(positions[b]))
                <= t2
            {
                remap.insert(b, a);
            }
        }
    }
    if remap.is_empty() {
        return mesh.clone();
    }
    rebuild(mesh, &remap, &HashMap::new())
}

fn resolve(remap: &HashMap<usize, usize>, mut v: usize) -> usize {
    while let Some(&n) = remap.get(&v) {
        if n == v {
            break;
        }
        v = n;
    }
    v
}

fn remap_faces(faces: &[Vec<usize>], remap: &HashMap<usize, usize>) -> Vec<Vec<usize>> {
    faces
        .iter()
        .filter_map(|f| {
            let mut r: Vec<usize> = Vec::with_capacity(f.len());
            for &v in f {
                let rv = resolve(remap, v);
                if r.last() != Some(&rv) {
                    r.push(rv);
                }
            }
            if r.len() >= 2 && r.first() == r.last() {
                r.pop();
            }
            (r.len() >= 3).then_some(r)
        })
        .collect()
}

/// Rebuild `mesh` applying `remap` (child → parent vertex) and any `moves`
/// (vertex → new position). Unreferenced vertices are compacted out.
fn rebuild(mesh: &Mesh, remap: &HashMap<usize, usize>, moves: &HashMap<usize, Vec3>) -> Mesh {
    let (mut positions, faces_src) = mesh.clone_positions_faces();
    for (&k, &p) in moves {
        if k < positions.len() {
            positions[k] = p;
        }
    }
    let faces = remap_faces(&faces_src, remap);

    // Compact: keep only referenced vertices.
    let mut used = vec![false; positions.len()];
    for f in &faces {
        for &v in f {
            used[v] = true;
        }
    }
    let mut new_idx = vec![usize::MAX; positions.len()];
    let mut compact_pos = Vec::new();
    for (i, u) in used.iter().enumerate() {
        if *u {
            new_idx[i] = compact_pos.len();
            compact_pos.push(positions[i]);
        }
    }
    let compact_faces: Vec<Vec<usize>> = faces
        .iter()
        .map(|f| f.iter().map(|&v| new_idx[v]).collect())
        .collect();
    Mesh::from_polygons(&compact_pos, &compact_faces)
}

/// A small extension so this module can get an owned `(positions, faces)` soup.
trait SoupView {
    fn clone_positions_faces(&self) -> (Vec<Vec3>, Vec<Vec<usize>>);
}
impl SoupView for Mesh {
    fn clone_positions_faces(&self) -> (Vec<Vec3>, Vec<Vec<usize>>) {
        (
            self.positions(),
            self.polygons()
                .iter()
                .map(|f| f.iter().map(|v| v.0).collect())
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn merge_two_cube_corners_at_center() {
        let m = primitives::cube(2.0);
        let before = m.vertex_count();
        let merged = merge_vertices(&m, &[VertexId(0), VertexId(1)], MergeTarget::Center);
        assert_eq!(merged.vertex_count(), before - 1);
        // Two faces of the cube collapse a corner and become triangles; still
        // a valid (if no longer closed) surface.
        assert!(merged.face_count() >= 4);
    }

    #[test]
    fn merge_at_point_places_the_vertex() {
        let m = primitives::grid(1, 1, 2.0);
        let p = Vec3::new(5.0, 5.0, 5.0);
        let merged = merge_vertices(&m, &[VertexId(0), VertexId(2)], MergeTarget::Point(p));
        // The surviving vertex is at p.
        assert!((0..merged.vertex_count()).any(|i| merged
            .vertex(VertexId(i))
            .unwrap()
            .position
            .sub(p)
            .length()
            < 1e-9));
    }

    #[test]
    fn merge_edges_collapses_a_grid_edge() {
        let m = primitives::grid(2, 2, 4.0);
        let before = m.vertex_count();
        let merged = merge_edges(&m, &[EdgeId(0)]);
        assert_eq!(merged.vertex_count(), before - 1);
    }

    #[test]
    fn merge_by_distance_within_a_subset() {
        // Grid with a duplicated vertex stacked on vertex 0.
        let g = primitives::grid(1, 1, 2.0);
        let mut positions = g.positions();
        let faces: Vec<Vec<usize>> = g
            .polygons()
            .iter()
            .map(|f| f.iter().map(|v| v.0).collect())
            .collect();
        positions.push(positions[0]); // exact duplicate, id 4
        let m = Mesh::from_polygons(&positions, &faces);
        // id 4 is unreferenced; merging {0,4} within threshold drops it.
        let merged = merge_by_distance(&m, &[VertexId(0), VertexId(4)], 1e-6);
        assert_eq!(merged.vertex_count(), 4);
    }

    #[test]
    fn merge_by_distance_keeps_far_apart_vertices() {
        let m = primitives::grid(2, 2, 4.0);
        let all: Vec<VertexId> = (0..m.vertex_count()).map(VertexId).collect();
        let merged = merge_by_distance(&m, &all, 0.01);
        assert_eq!(merged.vertex_count(), m.vertex_count());
    }
}
