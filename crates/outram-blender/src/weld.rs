// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Merge coincident vertices by spatial-hash bucketing plus union-find:
// R. E. Tarjan, "Efficiency of a Good But Not Linear Set Union Algorithm",
// Journal of the ACM 22(2), 1975, pp. 215-225.
// Written from the published formulation; no upstream source was copied.
// Blender analogue (architecture only): the bmo_remove_doubles operator
// (Merge by Distance).
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

//! Weld / remove-doubles — merge coincident vertices within a distance
//! tolerance into a single vertex.
//!
//! This is the pure-Rust analogue of Blender's **Merge by Distance**
//! (`bmesh` `bmo_remove_doubles` / the Weld modifier `MOD_weld`): vertices
//! closer than `distance` collapse to one, faces are rebuilt on the merged
//! vertex set, and any face that loses too many distinct corners (fewer than
//! 3) is dropped. It is the cleanup primitive that hardens a polygon soup — a
//! boolean result, an imported mesh, or an "exploded" face-varying surface —
//! into a watertight, shared-vertex mesh before it feeds the CSG / polyMesh
//! [`crate::export`] bridges.
//!
//! # What "coincident" means
//!
//! Two vertices are welded when the Euclidean distance between them is
//! `<= distance`. The merge relation is **not transitive** — `A`–`B` within
//! tolerance and `B`–`C` within tolerance does not imply `A`–`C` within
//! tolerance — so clusters are formed by **connectivity** (a union-find over
//! all within-tolerance pairs), not by pairwise distance from a single seed.
//! Every vertex in one connected cluster collapses to one output vertex placed
//! at the **average** of the cluster's positions (deterministic and
//! order-independent, matching the averaging convention used elsewhere in the
//! crate — Catmull-Clark, centroids).
//!
//! # Algorithm (O(n) expected)
//!
//! 1. **`distance <= 0`** — weld only *exactly* coincident vertices, keyed on
//!    the `f64` bit pattern. A no-op when the mesh has no exact duplicates.
//! 2. **`distance > 0`** — hash every vertex into a uniform grid of cell size
//!    `distance`. Because a within-tolerance partner can straddle a cell
//!    boundary, each vertex is tested against candidates in the **3×3×3 block
//!    of neighbour cells** (a cell size equal to the tolerance makes one cell
//!    of reach provably sufficient). Each accepted pair is `union`-ed.
//! 3. Average each cluster's positions, rebuild faces on the merged set
//!    (dropping consecutive-duplicate corners and any face left with `< 3`
//!    distinct vertices), and compact so no isolated vertices survive.
//!
//! No `faer`, no external dependency — just the fixed-size [`crate::math`]
//! types and standard collections. Android-safe.

use crate::math::Vec3;
use crate::mesh::Mesh;
use std::collections::HashMap;

/// Merge vertices closer than `distance` into one, returning the welded mesh.
///
/// `distance` is a Euclidean length in the mesh's coordinate units. A value of
/// `0.0` (or negative) welds only bit-identical duplicate vertices, so it is a
/// safe no-op on a mesh that has none. Faces whose corners collapse to fewer
/// than three distinct vertices after welding are discarded; the winding order
/// of every surviving face is preserved (only vertex identities are
/// substituted).
///
/// This is infallible: the result is always a valid mesh (possibly with fewer
/// vertices and faces than the input).
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, weld::weld};
///
/// // A pristine cube has no coincident vertices, so a zero-tolerance weld is a
/// // no-op and even a generous tolerance below the edge length changes nothing.
/// let cube = primitives::cube(2.0);
/// let welded = weld(&cube, 0.1);
/// assert_eq!(welded.vertex_count(), 8);
/// assert_eq!(welded.face_count(), 6);
/// assert_eq!(welded.euler_characteristic(), 2);
/// ```
pub fn weld(mesh: &Mesh, distance: f64) -> Mesh {
    let positions = mesh.positions();
    let polygons = mesh.polygons();
    let n = positions.len();
    if n == 0 {
        return Mesh::new();
    }

    let mut uf = UnionFind::new(n);

    if distance <= 0.0 {
        // Exact-match weld: bucket on the f64 bit pattern of the position.
        // (+0.0 and -0.0 hash to distinct keys — irrelevant for generated
        // meshes, which never produce a signed zero at a shared corner.)
        let mut exact: HashMap<(u64, u64, u64), usize> = HashMap::with_capacity(n);
        for (i, p) in positions.iter().enumerate() {
            let key = (p.x.to_bits(), p.y.to_bits(), p.z.to_bits());
            match exact.get(&key) {
                Some(&j) => uf.union(i, j),
                None => {
                    exact.insert(key, i);
                }
            }
        }
    } else {
        // Tolerance weld: uniform grid hash, cell size = distance, scan the
        // 3×3×3 neighbour block so boundary-straddling pairs are still found.
        let h = distance;
        let tol_sq = distance * distance;
        let cell_of = |p: &Vec3| -> (i64, i64, i64) {
            ((p.x / h).floor() as i64, (p.y / h).floor() as i64, (p.z / h).floor() as i64)
        };
        let mut grid: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
        for (i, p) in positions.iter().enumerate() {
            grid.entry(cell_of(p)).or_default().push(i);
        }
        for (i, p) in positions.iter().enumerate() {
            let (cx, cy, cz) = cell_of(p);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if let Some(bucket) = grid.get(&(cx + dx, cy + dy, cz + dz)) {
                            for &j in bucket {
                                // j < i pairs every distinct pair exactly once.
                                if j < i {
                                    let d = positions[i].sub(positions[j]);
                                    if d.dot(d) <= tol_sq {
                                        uf.union(i, j);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Cluster average positions, keyed by union-find root.
    let mut sum: HashMap<usize, (Vec3, usize)> = HashMap::new();
    for (i, p) in positions.iter().enumerate() {
        let root = uf.find(i);
        let entry = sum.entry(root).or_insert((Vec3::ZERO, 0));
        entry.0 = entry.0.add(*p);
        entry.1 += 1;
    }
    let cluster_pos: HashMap<usize, Vec3> = sum
        .into_iter()
        .map(|(root, (acc, count))| (root, acc.scale(1.0 / count as f64)))
        .collect();

    // Rebuild faces on the merged set, compacting to only vertices that survive
    // on some face (mirrors the decimate / convex-hull rebuild idiom).
    let mut new_positions: Vec<Vec3> = Vec::new();
    let mut remap: HashMap<usize, usize> = HashMap::new();
    let mut new_faces: Vec<Vec<usize>> = Vec::new();

    for poly in &polygons {
        // Map each corner to its cluster root, then drop consecutive
        // duplicates (including the wrap-around last == first) so a shared
        // welded corner does not leave a zero-length edge.
        let roots: Vec<usize> = poly.iter().map(|v| uf.find(v.0)).collect();
        let mut cleaned: Vec<usize> = Vec::with_capacity(roots.len());
        for &r in &roots {
            if cleaned.last() != Some(&r) {
                cleaned.push(r);
            }
        }
        if cleaned.len() > 1 && cleaned.first() == cleaned.last() {
            cleaned.pop();
        }
        // A face with fewer than 3 distinct corners has collapsed — drop it.
        if cleaned.len() < 3 {
            continue;
        }
        let face: Vec<usize> = cleaned
            .into_iter()
            .map(|root| {
                *remap.entry(root).or_insert_with(|| {
                    new_positions.push(cluster_pos[&root]);
                    new_positions.len() - 1
                })
            })
            .collect();
        new_faces.push(face);
    }

    Mesh::from_polygons(&new_positions, &new_faces)
}

/// Disjoint-set (union-find) with path compression and union by rank.
///
/// Used to group vertices into weld clusters by connectivity: because the
/// "within tolerance" relation is not transitive, pairwise merges must be
/// resolved into globally consistent clusters, which is exactly what a
/// disjoint-set computes independent of the order pairs are discovered.
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        UnionFind { parent: (0..n).collect(), rank: vec![0; n] }
    }

    fn find(&mut self, x: usize) -> usize {
        let mut root = x;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        // Path compression.
        let mut cur = x;
        while self.parent[cur] != root {
            let next = self.parent[cur];
            self.parent[cur] = root;
            cur = next;
        }
        root
    }

    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return;
        }
        match self.rank[ra].cmp(&self.rank[rb]) {
            std::cmp::Ordering::Less => self.parent[ra] = rb,
            std::cmp::Ordering::Greater => self.parent[rb] = ra,
            std::cmp::Ordering::Equal => {
                self.parent[rb] = ra;
                self.rank[ra] += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// Build an "exploded" mesh: every face gets its own private copy of each
    /// corner, so shared corners appear as several coincident vertices. This is
    /// the classic face-varying soup that weld must stitch back together.
    fn explode(mesh: &Mesh) -> Mesh {
        let ps = mesh.positions();
        let mut positions: Vec<Vec3> = Vec::new();
        let mut faces: Vec<Vec<usize>> = Vec::new();
        for poly in mesh.polygons() {
            let mut face = Vec::with_capacity(poly.len());
            for v in poly {
                positions.push(ps[v.0]);
                face.push(positions.len() - 1);
            }
            faces.push(face);
        }
        Mesh::from_polygons(&positions, &faces)
    }

    /// V&V — headline test. Methodology: explode a `cube(2.0)` into a 24-vertex
    /// face-varying soup (each of 8 corners duplicated 3×, one per incident
    /// face), then weld with a tolerance well below the edge length (2.0).
    /// Pass criterion: the soup collapses back to the canonical closed cube.
    /// Result: V 24→8, F 6, E 12, χ = 2 (watertight genus-0 surface).
    #[test]
    fn exploded_cube_welds_back_to_canonical() {
        let soup = explode(&primitives::cube(2.0));
        assert_eq!(soup.vertex_count(), 24, "exploded cube has 6 faces × 4 corners");
        let welded = weld(&soup, 0.5);
        assert_eq!(welded.vertex_count(), 8, "8 distinct corners survive");
        assert_eq!(welded.face_count(), 6);
        assert_eq!(welded.edge_count(), 12);
        assert_eq!(welded.euler_characteristic(), 2);
    }

    /// V&V — a jittered soup within tolerance still welds. Each duplicated
    /// corner is nudged by a random-ish sub-tolerance offset; the cluster
    /// average lands near the true corner and the counts match the clean weld.
    #[test]
    fn jittered_soup_within_tolerance_welds() {
        let ps = primitives::cube(2.0).positions();
        let polys = primitives::cube(2.0).polygons();
        let mut positions: Vec<Vec3> = Vec::new();
        let mut faces: Vec<Vec<usize>> = Vec::new();
        // Deterministic tiny jitter per copy, all < 0.05 in magnitude.
        let mut k = 0usize;
        for poly in &polys {
            let mut face = Vec::with_capacity(poly.len());
            for v in poly {
                let j = ((k % 7) as f64) * 0.004 - 0.012; // in [-0.012, 0.012]
                positions.push(ps[v.0].add(Vec3::new(j, -j, j)));
                face.push(positions.len() - 1);
                k += 1;
            }
            faces.push(face);
        }
        let soup = Mesh::from_polygons(&positions, &faces);
        let welded = weld(&soup, 0.2);
        assert_eq!(welded.vertex_count(), 8);
        assert_eq!(welded.face_count(), 6);
        assert_eq!(welded.euler_characteristic(), 2);
    }

    /// V&V — `distance == 0` is a safe no-op on a mesh with no exact
    /// duplicates. Pass criterion: identical topology and exact positions.
    #[test]
    fn zero_tolerance_is_noop_on_pristine_mesh() {
        let cube = primitives::cube(2.0);
        let before = cube.positions();
        let welded = weld(&cube, 0.0);
        assert_eq!(welded.vertex_count(), 8);
        assert_eq!(welded.face_count(), 6);
        assert_eq!(welded.euler_characteristic(), 2);
        // Positions preserved (order may compact but the set is identical).
        let after = welded.positions();
        for p in &before {
            assert!(
                after.iter().any(|q| q.sub(*p).length() < 1e-12),
                "every original corner must survive a zero-tolerance weld",
            );
        }
    }

    /// V&V — `distance == 0` still merges *exactly* coincident vertices. The
    /// exploded cube has bit-identical corner copies, so even tol 0 stitches it.
    #[test]
    fn zero_tolerance_merges_exact_duplicates() {
        let soup = explode(&primitives::cube(2.0));
        let welded = weld(&soup, 0.0);
        assert_eq!(welded.vertex_count(), 8, "exact duplicates collapse at tol 0");
        assert_eq!(welded.euler_characteristic(), 2);
    }

    /// V&V — non-transitive chain. Three collinear points A,B,C spaced 0.6·tol
    /// apart: A–B and B–C are within tol but A–C (1.2·tol) is not. Union-find
    /// groups all three by connectivity. We embed them as three near-coincident
    /// corners of one triangle inside a larger mesh and check they become one.
    #[test]
    fn non_transitive_chain_forms_one_cluster() {
        // Two triangles sharing an edge, with an extra vertex chain to weld.
        let tol = 0.1;
        let s = 0.6 * tol;
        let positions = vec![
            Vec3::new(0.0, 0.0, 0.0), // 0 (A)
            Vec3::new(s, 0.0, 0.0),   // 1 (B)  A–B = 0.6 tol
            Vec3::new(2.0 * s, 0.0, 0.0), // 2 (C) B–C = 0.6 tol, A–C = 1.2 tol
            Vec3::new(1.0, 0.0, 0.0), // 3
            Vec3::new(0.5, 1.0, 0.0), // 4
        ];
        // Faces reference A, B, C as if separate corners.
        let faces = vec![vec![0usize, 3, 4], vec![1, 3, 4], vec![2, 3, 4]];
        let soup = Mesh::from_polygons(&positions, &faces);
        let welded = weld(&soup, tol);
        // A, B, C (indices 0,1,2) all merge; 3 and 4 stay. So 3 vertices total.
        assert_eq!(welded.vertex_count(), 3, "A,B,C collapse via connectivity");
    }

    /// V&V — a face that collapses to fewer than 3 distinct corners is dropped,
    /// not passed to `add_face` (which would panic). A quad with two adjacent
    /// corners within tol becomes a triangle; a triangle with two welded
    /// corners disappears.
    #[test]
    fn degenerate_faces_are_dropped() {
        let tol = 0.1;
        // Vertices: 0,1 near-coincident; 2,3 distinct.
        let positions = vec![
            Vec3::new(0.0, 0.0, 0.0),  // 0
            Vec3::new(0.02, 0.0, 0.0), // 1 within tol of 0
            Vec3::new(1.0, 0.0, 0.0),  // 2
            Vec3::new(0.5, 1.0, 0.0),  // 3
        ];
        // Triangle [0,1,2] -> after weld only {01, 2} = 2 distinct -> dropped.
        // Quad [0,2,3,1] -> after weld {01, 2, 3} = 3 distinct -> kept as tri.
        let faces = vec![vec![0usize, 1, 2], vec![0, 2, 3, 1]];
        let soup = Mesh::from_polygons(&positions, &faces);
        let welded = weld(&soup, tol);
        assert_eq!(welded.face_count(), 1, "the collapsed triangle is dropped");
        assert_eq!(welded.vertex_count(), 3, "surviving quad-turned-tri has 3 corners");
    }

    /// V&V — the `MeshOp::Weld` dispatch matches the direct free-function call.
    #[test]
    fn meshop_weld_matches_free_function() {
        use crate::ops::MeshOp;
        let soup = explode(&primitives::cube(2.0));
        let direct = weld(&soup, 0.5);
        let via_op = MeshOp::Weld { distance: 0.5 }.apply(soup).expect("weld is infallible");
        assert_eq!(via_op.vertex_count(), direct.vertex_count());
        assert_eq!(via_op.face_count(), direct.face_count());
        assert_eq!(via_op.euler_characteristic(), direct.euler_characteristic());
    }
}
