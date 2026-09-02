// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Rip / Split / Separate. Follows the published behaviour of Blender's
// disconnect operators (source/blender/editors/mesh/editmesh_rip.cc,
// mesh_data.cc `MESH_OT_separate`, and `MESH_OT_split`,
// github.com/blender/blender, GPL-2.0-or-later): tear geometry apart along a
// selection, split a selection off as its own island, or separate it into a
// second mesh — by selection, by connected component, or by a caller group.
// Concepts only — no upstream source copied; polygon-soup rebuilds.
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

//! **Rip / Split / Separate** (`op-hzs.54.12`, GH issue #37 §B).
//!
//! - [`split_faces`] — disconnect the selected faces from the rest along their
//!   shared boundary (each shared vertex is duplicated), leaving one mesh with
//!   two islands that no longer share topology. Blender's `Y`.
//! - [`separate_selection`] — remove the selected faces from the mesh and
//!   return them as a **second** mesh. Blender's `P ▸ Selection`.
//! - [`separate_loose_parts`] — one mesh per connected component. Blender's
//!   `P ▸ By Loose Parts`.
//! - [`separate_by_group`] — one mesh per value of a caller-supplied
//!   per-face key (stands in for `P ▸ By Material` until material layers land
//!   in `op-hzs.54.28`).
//! - [`rip_edges`] — duplicate the vertices of the given interior edges and
//!   hand one side's faces the copies, tearing a slit. A minimal headless
//!   Rip; Rip Fill (capping the slit) composes it with
//!   [`crate::fill_holes`].

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::math::Vec3;
use crate::mesh::{EdgeId, FaceId, Mesh};

/// Disconnect the selected faces from the unselected ones: every vertex shared
/// by both groups is duplicated so the two groups no longer share it. Returns
/// one mesh with two now-independent islands (positions unchanged).
pub fn split_faces(mesh: &Mesh, faces: &[FaceId]) -> Mesh {
    let sel: HashSet<usize> = faces.iter().map(|f| f.0).filter(|&f| f < mesh.face_count()).collect();
    if sel.is_empty() || sel.len() == mesh.face_count() {
        return mesh.clone();
    }
    let polys = mesh.polygons();

    // Vertices used by a selected face and by an unselected face → duplicate.
    let mut used_sel: HashSet<usize> = HashSet::new();
    let mut used_other: HashSet<usize> = HashSet::new();
    for (fi, poly) in polys.iter().enumerate() {
        let tgt = if sel.contains(&fi) { &mut used_sel } else { &mut used_other };
        for v in poly {
            tgt.insert(v.0);
        }
    }
    let mut positions = mesh.positions();
    let mut dup: HashMap<usize, usize> = HashMap::new();
    for &v in used_sel.intersection(&used_other) {
        positions.push(positions[v]);
        dup.insert(v, positions.len() - 1);
    }

    let out: Vec<Vec<usize>> = polys
        .iter()
        .enumerate()
        .map(|(fi, poly)| {
            poly.iter()
                .map(|v| {
                    if sel.contains(&fi) {
                        dup.get(&v.0).copied().unwrap_or(v.0)
                    } else {
                        v.0
                    }
                })
                .collect()
        })
        .collect();
    Mesh::from_polygons(&positions, &out)
}

/// Remove the selected faces from `mesh` and return `(remaining, separated)`.
/// Each result is compacted to only its own vertices.
pub fn separate_selection(mesh: &Mesh, faces: &[FaceId]) -> (Mesh, Mesh) {
    let sel: HashSet<usize> = faces.iter().map(|f| f.0).collect();
    let polys = mesh.polygons();
    let take = |want_selected: bool| -> Mesh {
        let picked: Vec<Vec<usize>> = polys
            .iter()
            .enumerate()
            .filter(|(fi, _)| sel.contains(fi) == want_selected)
            .map(|(_, poly)| poly.iter().map(|v| v.0).collect())
            .collect();
        compact(&mesh.positions(), &picked)
    };
    (take(false), take(true))
}

/// Split `mesh` into one mesh per connected component (by shared edge).
pub fn separate_loose_parts(mesh: &Mesh) -> Vec<Mesh> {
    let polys = mesh.polygons();
    // Union-find over faces via shared undirected edges.
    let mut parent: Vec<usize> = (0..polys.len()).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    let mut edge_owner: HashMap<(usize, usize), usize> = HashMap::new();
    for (fi, poly) in polys.iter().enumerate() {
        let n = poly.len();
        for i in 0..n {
            let (a, b) = (poly[i].0, poly[(i + 1) % n].0);
            let key = (a.min(b), a.max(b));
            if let Some(&g) = edge_owner.get(&key) {
                let (ra, rb) = (find(&mut parent, fi), find(&mut parent, g));
                parent[ra] = rb;
            } else {
                edge_owner.insert(key, fi);
            }
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for fi in 0..polys.len() {
        let r = find(&mut parent, fi);
        groups.entry(r).or_default().push(fi);
    }
    let positions = mesh.positions();
    let mut out: Vec<Mesh> = groups
        .into_values()
        .map(|fs| {
            let picked: Vec<Vec<usize>> =
                fs.iter().map(|&fi| polys[fi].iter().map(|v| v.0).collect()).collect();
            compact(&positions, &picked)
        })
        .collect();
    out.sort_by_key(|m| std::cmp::Reverse(m.face_count()));
    out
}

/// Split `mesh` into one mesh per distinct value of `group(face)` — a
/// stand-in for *Separate by Material*. Returns `(key, mesh)` sorted by key.
pub fn separate_by_group(mesh: &Mesh, group: impl Fn(FaceId) -> usize) -> Vec<(usize, Mesh)> {
    let polys = mesh.polygons();
    let mut buckets: HashMap<usize, Vec<Vec<usize>>> = HashMap::new();
    for (fi, poly) in polys.iter().enumerate() {
        buckets
            .entry(group(FaceId(fi)))
            .or_default()
            .push(poly.iter().map(|v| v.0).collect());
    }
    let positions = mesh.positions();
    let mut out: Vec<(usize, Mesh)> =
        buckets.into_iter().map(|(k, fs)| (k, compact(&positions, &fs))).collect();
    out.sort_by_key(|(k, _)| *k);
    out
}

/// Tear a slit along `edges`: each edge's vertices are duplicated and the faces
/// on **one** side of the edge (the second incident face, by id) get the
/// copies. An interior edge becomes an open slit; pair with
/// [`crate::fill_holes`] for Rip Fill.
pub fn rip_edges(mesh: &Mesh, edges: &[EdgeId]) -> Mesh {
    let rip: BTreeSet<EdgeId> = edges.iter().copied().collect();
    let polys = mesh.polygons();
    let mut positions = mesh.positions();

    // For each ripped edge, the "far side" face (second by id) and a per-vertex
    // duplicate used only by that face and anything else on its side.
    let mut dup: HashMap<usize, usize> = HashMap::new();
    let mut moved_faces: HashSet<usize> = HashSet::new();
    for &e in &rip {
        let Some(ed) = mesh.edge(e) else { continue };
        let mut incident: Vec<usize> = Vec::new();
        for (fi, poly) in polys.iter().enumerate() {
            let n = poly.len();
            if (0..n).any(|i| {
                let (a, b) = (poly[i].0, poly[(i + 1) % n].0);
                (a == ed.verts[0].0 && b == ed.verts[1].0) || (a == ed.verts[1].0 && b == ed.verts[0].0)
            }) {
                incident.push(fi);
            }
        }
        if incident.len() < 2 {
            continue; // boundary edge: nothing to rip
        }
        let far = *incident.iter().max().unwrap();
        moved_faces.insert(far);
        for v in [ed.verts[0].0, ed.verts[1].0] {
            dup.entry(v).or_insert_with(|| {
                positions.push(positions[v]);
                positions.len() - 1
            });
        }
    }

    let out: Vec<Vec<usize>> = polys
        .iter()
        .enumerate()
        .map(|(fi, poly)| {
            poly.iter()
                .map(|v| {
                    if moved_faces.contains(&fi) {
                        dup.get(&v.0).copied().unwrap_or(v.0)
                    } else {
                        v.0
                    }
                })
                .collect()
        })
        .collect();
    Mesh::from_polygons(&positions, &out)
}

/// Rebuild a soup keeping only referenced vertices, re-indexed from 0.
fn compact(positions: &[Vec3], faces: &[Vec<usize>]) -> Mesh {
    let mut used = vec![false; positions.len()];
    for f in faces {
        for &v in f {
            if v < used.len() {
                used[v] = true;
            }
        }
    }
    let mut idx = vec![usize::MAX; positions.len()];
    let mut pos = Vec::new();
    for (i, u) in used.iter().enumerate() {
        if *u {
            idx[i] = pos.len();
            pos.push(positions[i]);
        }
    }
    let f: Vec<Vec<usize>> = faces.iter().map(|face| face.iter().map(|&v| idx[v]).collect()).collect();
    Mesh::from_polygons(&pos, &f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn split_faces_disconnects_a_grid_patch() {
        let m = primitives::grid(3, 3, 3.0); // 9 quads, 16 verts
        let before = m.vertex_count();
        let s = split_faces(&m, &[FaceId(0)]);
        // FaceId(0)'s two shared-boundary verts (interior of the grid) get
        // duplicated.
        assert!(s.vertex_count() > before);
        assert_eq!(s.face_count(), m.face_count());
    }

    #[test]
    fn separate_selection_partitions_the_faces() {
        let m = primitives::cube(2.0);
        let (rest, taken) = separate_selection(&m, &[FaceId(0), FaceId(1)]);
        assert_eq!(rest.face_count(), 4);
        assert_eq!(taken.face_count(), 2);
        assert!(taken.vertex_count() <= 8);
    }

    #[test]
    fn separate_loose_parts_of_two_cubes() {
        let a = primitives::cube(2.0);
        let mut positions = a.positions();
        let mut faces: Vec<Vec<usize>> =
            a.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect();
        let off = positions.len();
        for p in a.positions() {
            positions.push(p.add(Vec3::new(10.0, 0.0, 0.0)));
        }
        for f in a.polygons() {
            faces.push(f.iter().map(|v| v.0 + off).collect());
        }
        let m = Mesh::from_polygons(&positions, &faces);
        let parts = separate_loose_parts(&m);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].face_count(), 6);
        assert_eq!(parts[1].face_count(), 6);
    }

    #[test]
    fn separate_by_group_splits_a_grid_in_half() {
        let m = primitives::grid(4, 1, 4.0); // 4 faces
        let parts = separate_by_group(&m, |f| if f.0 < 2 { 0 } else { 1 });
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].1.face_count(), 2);
        assert_eq!(parts[1].1.face_count(), 2);
    }

    #[test]
    fn rip_an_interior_grid_edge_opens_a_slit() {
        let m = primitives::grid(2, 2, 4.0);
        let topo = crate::topology::MeshTopology::new(&m);
        let interior = (0..m.edge_count()).map(EdgeId).find(|&e| topo.is_manifold_edge(e)).unwrap();
        let r = rip_edges(&m, &[interior]);
        assert!(r.vertex_count() > m.vertex_count(), "ripped edge's verts duplicated");
        assert_eq!(r.face_count(), m.face_count());
    }
}
