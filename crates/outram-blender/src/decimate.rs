// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Implements: M. Garland and P. S. Heckbert, "Surface Simplification Using Quadric
// Error Metrics", SIGGRAPH '97, pp. 209-216.
// Written from the published formulation; no upstream source was copied.
// Blender analogue (architecture only): the Decimate modifier, Collapse mode.
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

//! **QEM mesh decimation** — Garland–Heckbert quadric-error-metric edge-collapse
//! simplification.
//!
//! Blender analogue: the **Decimate** modifier (`MOD_decimate`, "Collapse"
//! mode). Reduces a mesh's triangle count while preserving its shape, by
//! repeatedly collapsing the edge whose removal introduces the least
//! *quadric error* (squared distance to the planes of the original surface).
//!
//! ## The method (Garland & Heckbert, 1997)
//!
//! Each vertex carries a `4×4` symmetric **error quadric** `Q` — the sum of the
//! outer products `p pᵀ` of the plane equations `p = (a, b, c, d)` of its
//! incident triangles (area-weighted). The squared distance of a point `v` to
//! all those planes is `vᵀ Q v`. To collapse edge `(i, j)` we combine
//! `Q = Q_i + Q_j`, place the merged vertex at the `v` minimizing `vᵀ Q v` (a
//! `3×3` solve, with a fallback to the cheaper of the endpoints/midpoint when
//! that is singular), and use that minimum as the collapse **cost**. A min-heap
//! keyed on cost drives a greedy sequence of collapses until a target face count
//! is reached.
//!
//! Guards keep the result sane: a **link-condition** check rejects collapses
//! that would make the mesh non-manifold, a **normal-flip** check rejects those
//! that would fold a triangle over, and **boundary** edges get a large penalty
//! quadric so an open border is preserved.
//!
//! Everything here is closed-form (a hand-written symmetric-`3×3` solve and a
//! version-stamped lazy-deletion heap); no `faer` is needed. Works on the
//! polygon-soup view and rebuilds via [`Mesh::from_polygons`] — no half-edge
//! surgery.
//!
//! > **Untrusted AI-generated draft** until a human reviews it, per the
//! > workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
//! > reactor control, safety-critical, or licensing decisions.

use crate::math::Vec3;
use crate::mesh::{FaceId, Mesh};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// A symmetric `4×4` error quadric, stored as its 10 unique coefficients in the
/// order `[a², ab, ac, ad, b², bc, bd, c², cd, d²]`.
type Quadric = [f64; 10];

/// Why [`decimate`] stopped collapsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The requested target face count was reached.
    ReachedTarget,
    /// No legal collapse remained (every candidate was rejected by the manifold
    /// / flip / boundary guards) before the target was reached.
    NoLegalCollapse,
}

/// The result of a decimation: the simplified mesh and why it stopped.
#[derive(Debug, Clone)]
pub struct DecimateResult {
    /// The simplified mesh.
    pub mesh: Mesh,
    /// Why decimation stopped (target reached, or ran out of legal collapses).
    pub stop_reason: StopReason,
}

/// Simplify `mesh` down to roughly `target_faces` triangles by QEM edge
/// collapse, returning the simplified [`Mesh`].
///
/// Convenience wrapper over [`decimate_with_reason`] that discards the stop
/// reason. `target_faces` is a *lower bound goal*: the result has at most a
/// couple more faces than the target (collapses remove faces in pairs), or more
/// if no further legal collapse exists. Non-triangular faces are fan-triangulated
/// first, so the output is a triangle mesh.
pub fn decimate(mesh: &Mesh, target_faces: usize) -> Mesh {
    decimate_with_reason(mesh, target_faces).mesh
}

/// Like [`decimate`] but also reports the [`StopReason`].
pub fn decimate_with_reason(mesh: &Mesh, target_faces: usize) -> DecimateResult {
    let mut d = Decimator::from_mesh(mesh);
    let reason = d.run(target_faces);
    DecimateResult { mesh: d.rebuild(), stop_reason: reason }
}

// ---------------------------------------------------------------------------
// Quadric algebra.
// ---------------------------------------------------------------------------

/// The fundamental quadric `p pᵀ` of the plane `(a, b, c, d)`.
fn quadric_from_plane(a: f64, b: f64, c: f64, d: f64) -> Quadric {
    [a * a, a * b, a * c, a * d, b * b, b * c, b * d, c * c, c * d, d * d]
}

/// Element-wise sum of two quadrics.
fn quadric_add(p: Quadric, q: Quadric) -> Quadric {
    let mut r = [0.0; 10];
    for k in 0..10 {
        r[k] = p[k] + q[k];
    }
    r
}

/// Scale a quadric by `s`.
fn quadric_scale(p: Quadric, s: f64) -> Quadric {
    let mut r = [0.0; 10];
    for k in 0..10 {
        r[k] = p[k] * s;
    }
    r
}

/// `vᵀ Q v` for the homogeneous point `(v.x, v.y, v.z, 1)` — the squared
/// distance of `v` to the quadric's accumulated planes (clamped `≥ 0`).
fn quadric_error(q: &Quadric, v: Vec3) -> f64 {
    let (x, y, z) = (v.x, v.y, v.z);
    let e = q[0] * x * x
        + q[4] * y * y
        + q[7] * z * z
        + 2.0 * (q[1] * x * y + q[2] * x * z + q[5] * y * z)
        + 2.0 * (q[3] * x + q[6] * y + q[8] * z)
        + q[9];
    e.max(0.0)
}

/// The point minimizing `vᵀ Q v` (the top-left `3×3` system `A v = −b`), with a
/// fallback to the cheapest of `a`, `b`, and their midpoint when `A` is
/// singular. Returns `(target, cost)`.
fn optimal_position(q: &Quadric, a: Vec3, b: Vec3) -> (Vec3, f64) {
    // Symmetric 3×3 A = [[q0,q1,q2],[q1,q4,q5],[q2,q5,q7]], rhs = −(q3,q6,q8).
    let (q0, q1, q2, q4, q5, q7) = (q[0], q[1], q[2], q[4], q[5], q[7]);
    let det = q0 * (q4 * q7 - q5 * q5) - q1 * (q1 * q7 - q5 * q2) + q2 * (q1 * q5 - q4 * q2);
    let scale = (q0 + q4 + q7).abs().max(1.0);
    if det.abs() > 1e-10 * scale {
        // Symmetric adjugate.
        let c00 = q4 * q7 - q5 * q5;
        let c01 = -(q1 * q7 - q5 * q2);
        let c02 = q1 * q5 - q4 * q2;
        let c11 = q0 * q7 - q2 * q2;
        let c12 = -(q0 * q5 - q1 * q2);
        let c22 = q0 * q4 - q1 * q1;
        let (r0, r1, r2) = (-q[3], -q[6], -q[8]);
        let inv = 1.0 / det;
        let v = Vec3::new(
            (c00 * r0 + c01 * r1 + c02 * r2) * inv,
            (c01 * r0 + c11 * r1 + c12 * r2) * inv,
            (c02 * r0 + c12 * r1 + c22 * r2) * inv,
        );
        (v, quadric_error(q, v))
    } else {
        let mid = a.add(b).scale(0.5);
        [a, b, mid]
            .into_iter()
            .map(|c| (c, quadric_error(q, c)))
            .min_by(|x, y| x.1.total_cmp(&y.1))
            .expect("three candidates")
    }
}

/// Area-weighted plane quadric of triangle `(a, b, c)`, or `None` if degenerate
/// (zero area).
fn triangle_quadric(a: Vec3, b: Vec3, c: Vec3) -> Option<Quadric> {
    let n_raw = b.sub(a).cross(c.sub(a));
    let len = n_raw.length();
    if len <= f64::MIN_POSITIVE {
        return None;
    }
    let area = 0.5 * len;
    let n = n_raw.scale(1.0 / len);
    let d = -n.dot(a);
    Some(quadric_scale(quadric_from_plane(n.x, n.y, n.z, d), area))
}

/// Unit normal of a triangle, or [`Vec3::ZERO`] if degenerate.
fn tri_normal(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    b.sub(a).cross(c.sub(a)).normalize()
}

// ---------------------------------------------------------------------------
// The decimator.
// ---------------------------------------------------------------------------

/// One candidate edge collapse in the priority queue (min-heap on `cost`).
#[derive(Debug, Clone, Copy)]
struct Entry {
    cost: f64,
    i: usize,
    j: usize,
    vi: u64,
    vj: u64,
    target: Vec3,
}
impl PartialEq for Entry {
    fn eq(&self, o: &Self) -> bool {
        self.cost == o.cost
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Entry {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.cost.total_cmp(&o.cost)
    }
}

/// Mutable decimation state over the polygon soup.
struct Decimator {
    positions: Vec<Vec3>,
    quadrics: Vec<Quadric>,
    tris: Vec<[usize; 3]>,
    alive_v: Vec<bool>,
    alive_f: Vec<bool>,
    version: Vec<u64>,
    adj: Vec<HashSet<usize>>,
    vfaces: Vec<HashSet<usize>>,
    live_faces: usize,
}

/// Cosine threshold below which a collapse is treated as flipping a triangle.
const FLIP_COS: f64 = 0.2;
/// Weight of the perpendicular boundary-constraint quadric (freezes the border).
const BOUNDARY_WEIGHT: f64 = 1000.0;

impl Decimator {
    fn from_mesh(mesh: &Mesh) -> Decimator {
        let positions = mesh.positions();
        let n = positions.len();

        // Fan-triangulate every face.
        let mut tris: Vec<[usize; 3]> = Vec::new();
        for f in 0..mesh.face_count() {
            let vs = mesh.face_vertices(FaceId(f));
            if vs.len() < 3 {
                continue;
            }
            for k in 1..vs.len() - 1 {
                tris.push([vs[0].0, vs[k].0, vs[k + 1].0]);
            }
        }

        // Per-vertex quadrics (area-weighted face planes) + adjacency + vfaces.
        let mut quadrics = vec![[0.0f64; 10]; n];
        let mut adj: Vec<HashSet<usize>> = vec![HashSet::new(); n];
        let mut vfaces: Vec<HashSet<usize>> = vec![HashSet::new(); n];
        let mut edge_tris: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
        for (ti, &[a, b, c]) in tris.iter().enumerate() {
            if let Some(q) = triangle_quadric(positions[a], positions[b], positions[c]) {
                quadrics[a] = quadric_add(quadrics[a], q);
                quadrics[b] = quadric_add(quadrics[b], q);
                quadrics[c] = quadric_add(quadrics[c], q);
            }
            for &(u, w) in &[(a, b), (b, c), (c, a)] {
                adj[u].insert(w);
                adj[w].insert(u);
                edge_tris.entry(edge_key(u, w)).or_default().push(ti);
            }
            vfaces[a].insert(ti);
            vfaces[b].insert(ti);
            vfaces[c].insert(ti);
        }

        // Boundary edges (one incident triangle): add a perpendicular penalty
        // quadric to both endpoints so the border is preserved.
        for (&(u, w), ts) in &edge_tris {
            if ts.len() != 1 {
                continue;
            }
            let tri = tris[ts[0]];
            let nf = tri_normal(positions[tri[0]], positions[tri[1]], positions[tri[2]]);
            let e = positions[w].sub(positions[u]);
            let np = e.cross(nf).normalize();
            if np == Vec3::ZERO {
                continue;
            }
            let dp = -np.dot(positions[u]);
            let kb = quadric_scale(quadric_from_plane(np.x, np.y, np.z, dp), BOUNDARY_WEIGHT);
            quadrics[u] = quadric_add(quadrics[u], kb);
            quadrics[w] = quadric_add(quadrics[w], kb);
        }

        let live_faces = tris.len();
        Decimator {
            positions,
            quadrics,
            tris,
            alive_v: vec![true; n],
            alive_f: vec![true; live_faces],
            version: vec![0; n],
            adj,
            vfaces,
            live_faces,
        }
    }

    /// Optimal target + cost for collapsing edge `(i, j)`.
    fn optimal(&self, i: usize, j: usize) -> (Vec3, f64) {
        let q = quadric_add(self.quadrics[i], self.quadrics[j]);
        optimal_position(&q, self.positions[i], self.positions[j])
    }

    /// The set of vertices opposite edge `(i, j)` — third vertices of the live
    /// triangles that contain both `i` and `j` (these die on collapse).
    fn shared_opposites(&self, i: usize, j: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for &t in self.vfaces[i].intersection(&self.vfaces[j]) {
            if !self.alive_f[t] {
                continue;
            }
            for &v in &self.tris[t] {
                if v != i && v != j {
                    out.push(v);
                }
            }
        }
        out
    }

    /// Manifold link condition + normal-flip guard for collapsing `(i, j)` to
    /// `target`.
    fn is_legal(&self, i: usize, j: usize, target: Vec3) -> bool {
        // Link condition: the common neighbours of i and j must be exactly the
        // vertices opposite the shared (deleted) triangles. Any extra common
        // neighbour means the collapse would fold two sheets → non-manifold.
        let common = self.adj[i].intersection(&self.adj[j]).count();
        let opposites = self.shared_opposites(i, j);
        if common != opposites.len() {
            return false;
        }

        // Normal-flip: every surviving face incident to i or j (not one of the
        // deleted shared faces) must keep its orientation under i,j → target.
        for &t in self.vfaces[i].union(&self.vfaces[j]) {
            if !self.alive_f[t] {
                continue;
            }
            let tri = self.tris[t];
            let is_shared = tri.contains(&i) && tri.contains(&j);
            if is_shared {
                continue; // this face is removed by the collapse
            }
            let map = |v: usize| if v == i || v == j { target } else { self.positions[v] };
            let before = tri_normal(self.positions[tri[0]], self.positions[tri[1]], self.positions[tri[2]]);
            let after = tri_normal(map(tri[0]), map(tri[1]), map(tri[2]));
            if after == Vec3::ZERO || before.dot(after) < FLIP_COS {
                return false;
            }
        }
        true
    }

    /// Collapse edge `(i, j)`: merge `j` into `i` at `target`, retarget faces,
    /// update adjacency and quadric, and bump affected versions.
    fn collapse(&mut self, i: usize, j: usize, target: Vec3) {
        self.positions[i] = target;
        self.quadrics[i] = quadric_add(self.quadrics[i], self.quadrics[j]);
        self.alive_v[j] = false;

        // Retarget every face of j (and check i's faces) for j → i, dropping
        // faces that become degenerate (a repeated vertex).
        let affected: Vec<usize> = self.vfaces[j].union(&self.vfaces[i]).copied().collect();
        for t in affected {
            if !self.alive_f[t] {
                continue;
            }
            let mut tri = self.tris[t];
            for v in tri.iter_mut() {
                if *v == j {
                    *v = i;
                }
            }
            self.tris[t] = tri;
            let degenerate = tri[0] == tri[1] || tri[1] == tri[2] || tri[0] == tri[2];
            if degenerate {
                self.alive_f[t] = false;
                self.live_faces -= 1;
                for &v in &tri {
                    self.vfaces[v].remove(&t);
                }
            } else {
                self.vfaces[i].insert(t);
            }
        }

        // Adjacency: fold j's neighbours into i.
        let jn: Vec<usize> = self.adj[j].iter().copied().collect();
        for k in jn {
            self.adj[k].remove(&j);
            if k != i {
                self.adj[k].insert(i);
                self.adj[i].insert(k);
            }
        }
        self.adj[i].remove(&j);
        self.adj[i].remove(&i);
        self.adj[j].clear();

        // Bump versions: i and all its (new) neighbours have changed costs.
        self.version[i] += 1;
        let neighbours: Vec<usize> = self.adj[i].iter().copied().collect();
        for k in neighbours {
            self.version[k] += 1;
        }
    }

    /// Run the greedy collapse loop down to `target_faces`.
    fn run(&mut self, target_faces: usize) -> StopReason {
        let mut heap: BinaryHeap<Reverse<Entry>> = BinaryHeap::new();

        // Seed with every unique edge.
        let mut seen: HashSet<(usize, usize)> = HashSet::new();
        for t in 0..self.tris.len() {
            if !self.alive_f[t] {
                continue;
            }
            let [a, b, c] = self.tris[t];
            for &(u, w) in &[(a, b), (b, c), (c, a)] {
                if seen.insert(edge_key(u, w)) {
                    self.push_edge(&mut heap, u, w);
                }
            }
        }

        while self.live_faces > target_faces {
            let Some(Reverse(e)) = heap.pop() else {
                return StopReason::NoLegalCollapse;
            };
            // Lazy-deletion / validity checks.
            if !self.alive_v[e.i] || !self.alive_v[e.j] {
                continue;
            }
            if e.vi != self.version[e.i] || e.vj != self.version[e.j] {
                continue; // stale
            }
            if !self.adj[e.i].contains(&e.j) {
                continue; // edge gone
            }
            if !self.is_legal(e.i, e.j, e.target) {
                continue;
            }
            self.collapse(e.i, e.j, e.target);
            // Re-push i's star with fresh versions.
            let star: Vec<usize> = self.adj[e.i].iter().copied().collect();
            for k in star {
                self.push_edge(&mut heap, e.i, k);
            }
        }
        StopReason::ReachedTarget
    }

    /// Compute and push edge `(u, w)`'s current collapse entry.
    fn push_edge(&self, heap: &mut BinaryHeap<Reverse<Entry>>, u: usize, w: usize) {
        if u == w || !self.alive_v[u] || !self.alive_v[w] {
            return;
        }
        let (target, cost) = self.optimal(u, w);
        let (i, j) = (u.min(w), u.max(w));
        heap.push(Reverse(Entry { cost, i, j, vi: self.version[i], vj: self.version[j], target }));
    }

    /// Rebuild a compacted [`Mesh`] from the surviving triangles (dropping dead
    /// and isolated vertices).
    fn rebuild(&self) -> Mesh {
        let mut remap: HashMap<usize, usize> = HashMap::new();
        let mut positions: Vec<Vec3> = Vec::new();
        let mut faces: Vec<Vec<usize>> = Vec::new();
        for (t, &alive) in self.alive_f.iter().enumerate() {
            if !alive {
                continue;
            }
            let tri = self.tris[t];
            if tri[0] == tri[1] || tri[1] == tri[2] || tri[0] == tri[2] {
                continue;
            }
            // Skip a zero-area (collinear) triangle so from_polygons never sees one.
            if tri_normal(self.positions[tri[0]], self.positions[tri[1]], self.positions[tri[2]])
                == Vec3::ZERO
            {
                continue;
            }
            let mut face = Vec::with_capacity(3);
            for &v in &tri {
                let idx = *remap.entry(v).or_insert_with(|| {
                    positions.push(self.positions[v]);
                    positions.len() - 1
                });
                face.push(idx);
            }
            faces.push(face);
        }
        Mesh::from_polygons(&positions, &faces)
    }
}

/// Undirected edge key `(min, max)`.
fn edge_key(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// `true` if every undirected edge of the mesh borders exactly two faces.
    fn is_watertight(m: &Mesh) -> bool {
        let mut edge_use: HashMap<(usize, usize), usize> = HashMap::new();
        for face in m.polygons() {
            let n = face.len();
            for i in 0..n {
                *edge_use.entry(edge_key(face[i].0, face[(i + 1) % n].0)).or_insert(0) += 1;
            }
        }
        edge_use.values().all(|&c| c == 2)
    }

    /// Methodology: **a sphere stays closed and round.** Decimate a
    /// `uv_sphere(24, 12)` to ~half its faces. The result must remain a closed,
    /// watertight, genus-0 mesh (`chi = 2`, every edge shared by two faces) with
    /// a reduced face count, and every surviving vertex must stay within 10% of
    /// the original radius (QEM moves collapse points slightly off the exact
    /// sphere).
    ///
    /// Results (asserted): `chi = 2`, watertight, face count in `[0.4, 0.55]×`
    /// the original, all surviving vertices within `0.10·R` of the fitted radius.
    #[test]
    fn decimate_sphere_stays_closed_and_round() {
        let sphere = primitives::uv_sphere(24, 12, 1.0);
        let f0 = sphere.face_count();
        let target = f0 / 2;
        let out = decimate(&sphere, target);

        assert_eq!(out.euler_characteristic(), 2, "decimated sphere must stay closed (chi=2)");
        assert!(is_watertight(&out), "decimated sphere must stay watertight");
        assert!(out.face_count() < f0, "face count must drop: {} -> {}", f0, out.face_count());
        assert!(out.face_count() as f64 <= 0.55 * f0 as f64, "should approach the target");

        // Fitted centre/radius of the original, then check survivors.
        let op = sphere.positions();
        let c = op.iter().fold(Vec3::ZERO, |a, &p| a.add(p)).scale(1.0 / op.len() as f64);
        let r = op.iter().map(|p| p.sub(c).length()).sum::<f64>() / op.len() as f64;
        for p in out.positions() {
            let dr = (p.sub(c).length() - r).abs();
            assert!(dr <= 0.10 * r, "vertex drifted off the sphere: dr={dr}, r={r}");
        }
    }

    /// Methodology: **a flat grid stays planar and bounded.** Decimate a flat
    /// `grid(8, 8)` (in `z = 0`, footprint `[-0.5, 0.5]²`). With coplanar
    /// interior quadrics and boundary penalties, every collapse is free and
    /// stays in-plane and inside the footprint. The patch remains a disc.
    ///
    /// Results (asserted): reduced face count; every surviving vertex has
    /// `|z| < 1e-9` and lies within the original `[-0.5, 0.5]²` footprint;
    /// `chi = 1` (still a disc).
    #[test]
    fn decimate_flat_grid_stays_planar() {
        let grid = primitives::grid(8, 8, 1.0);
        let f0 = grid.face_count();
        let out = decimate(&grid, 20);

        assert!(out.face_count() < f0, "face count must drop: {} -> {}", f0, out.face_count());
        assert_eq!(out.euler_characteristic(), 1, "flat grid stays a disc (chi=1)");
        for p in out.positions() {
            assert!(p.z.abs() < 1e-9, "vertex left the z=0 plane: {}", p.z);
            assert!(p.x >= -0.5 - 1e-9 && p.x <= 0.5 + 1e-9, "x out of footprint: {}", p.x);
            assert!(p.y >= -0.5 - 1e-9 && p.y <= 0.5 + 1e-9, "y out of footprint: {}", p.y);
        }
    }

    /// Methodology: **count reaches the target + compaction is clean.** Decimate
    /// a sphere to an explicit small target and confirm the stop reason and that
    /// the rebuilt mesh has no isolated/dead vertices (every vertex is referenced
    /// by a face).
    ///
    /// Results (asserted): stop reason is `ReachedTarget`, face count `<= target
    /// + 2` (collapses remove faces in pairs), and every vertex index appears in
    /// some face.
    #[test]
    fn decimate_reaches_target_and_compacts() {
        let sphere = primitives::uv_sphere(16, 8, 1.0);
        let target = 40;
        let res = decimate_with_reason(&sphere, target);
        assert_eq!(res.stop_reason, StopReason::ReachedTarget);
        assert!(res.mesh.face_count() <= target + 2, "face count {} near target {target}", res.mesh.face_count());

        let mut referenced = vec![false; res.mesh.vertex_count()];
        for face in res.mesh.polygons() {
            for v in face {
                referenced[v.0] = true;
            }
        }
        assert!(referenced.iter().all(|&r| r), "no isolated vertices after compaction");
    }

    /// Methodology: **coplanar collapse is zero-cost** — the sharpest check of
    /// the quadric math. Build a flat `grid(3, 3)`; the combined quadric of any
    /// interior edge is a single repeated plane, so the optimal collapse point
    /// has zero squared distance.
    ///
    /// Results (asserted): the minimum edge-collapse cost over the grid is
    /// `<= 1e-9`, and its optimal target lies in the `z = 0` plane.
    #[test]
    fn coplanar_collapse_is_zero_cost() {
        let grid = primitives::grid(3, 3, 1.0);
        let d = Decimator::from_mesh(&grid);
        // Find the cheapest interior (non-boundary-dominated) edge cost.
        let mut best = f64::INFINITY;
        let mut best_target = Vec3::ZERO;
        for i in 0..d.positions.len() {
            for &j in &d.adj[i] {
                if i < j {
                    let (t, cost) = d.optimal(i, j);
                    if cost < best {
                        best = cost;
                        best_target = t;
                    }
                }
            }
        }
        assert!(best <= 1e-9, "a coplanar collapse must be ~zero cost, got {best}");
        assert!(best_target.z.abs() <= 1e-9, "optimal point must stay in-plane");
    }
}
