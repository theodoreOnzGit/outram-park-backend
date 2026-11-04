// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/smoothers/geometry/meshOptimizer/optimizeMeshFV.C
//     (meshOptimizer::untangleMeshFV, optimizeLowQualityFaces)
//   meshLibrary/utilities/meshes/partTetMesh/{partTetMesh.C,
//     partTetMeshAddressing.C, partTetMeshSimplex.C}
//   meshLibrary/utilities/smoothers/geometry/meshOptimizer/tetMeshOptimisation/
//     tetMeshOptimisation.C (optimiseUsingKnuppMetric,
//     optimiseUsingVolumeOptimizer)
//     advancedSmoothers/simplexSmoother/simplexSmoother.C
//     advancedSmoothers/volumeOptimizer/{volumeOptimizer.C,
//       volumeOptimizerEvaluateGradients.C}
//     advancedSmoothers/knuppMetric/knuppMetric.C
//   meshLibrary/utilities/meshes/polyMeshGenChecks/polyMeshGenChecksGeometry.C
//     (findBadFaces, findBadFacesRelaxed, findLowQualityFaces,
//      checkFaceDotProduct, checkFaceSkewness, checkFacePyramids,
//      checkFaceFlatness, checkCellPartTetrahedra, checkFaceAreas)
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae (2026-02-17). The crate's other
// files cite wyldckat/cfMesh 6cf1d211f9cf (v1.0, 2014), which has no
// boundaryLayerOptimisation; the files ported here are unchanged in kind
// between the two lines (meshOptimizer and partTetMesh exist in both).
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
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

//! cfMesh's **volume mesh optimiser** (gh:#1215): move interior points to
//! repair bad and low-quality faces, without changing the topology.
//!
//! The pieces, each a port of the cfMesh routine named:
//!
//! - [`find_bad_faces`] / [`find_low_quality_faces`]
//!   (`polyMeshGenChecks::findBadFaces`, `findBadFacesRelaxed`,
//!   `findLowQualityFaces`): which faces need work. *Bad* = a face pyramid
//!   pointing the wrong way, a face flatness below 0.8, a non-positive
//!   decomposition tetrahedron of a cell, or a vanishing face area. *Low
//!   quality* = non-orthogonality above 65° or cfMesh skewness above 2. The
//!   `meshQualitySettings` dictionary checks are not ported (no `meshDict`).
//! - [`PartTetMesh`] (`partTetMesh`): the cells around those faces, grown by
//!   some layers of point neighbours, decomposed into tetrahedra about their
//!   face and cell centres. A mesh point may move only when every face around
//!   it is inside the region and it is neither on the boundary nor locked.
//! - the per-point smoothers on a point's simplex (`partTetMeshSimplex`):
//!   [`volume_optimizer`] (`volumeOptimizer::optimizeNodePosition`: a
//!   divide-and-conquer search of the simplex's bounding box, then Newton on
//!   the stabilised Frobenius/volume functional) and [`knupp_metric`]
//!   (`knuppMetric::optimizeNodePosition`: push the point to the positive
//!   side of every opposite face).
//! - [`untangle_mesh_fv`] (`meshOptimizer::untangleMeshFV`) and
//!   [`optimize_low_quality_faces`] (`meshOptimizer::optimizeLowQualityFaces`).
//!
//! **Not ported, said so where it matters:** the LP-based `meshUntangler`
//! (`tmo.optimiseUsingMeshUntangler()`, the middle of the three smoothers in
//! `untangleMeshFV`), the boundary-point moves of `untangleMeshFV`'s second
//! loop (`optimiseBoundaryVolumeOptimizer`, `optimiseBoundarySurfaceLaplace`:
//! boundary points never move here), `tetMatcher` (a tetrahedral cell is
//! decomposed like any other; this crate's tet-dual meshes have none), and
//! the parallel buffer layers. Centres are this crate's: the face centre is
//! the vertex average and the cell centre the pyramid-decomposition centroid
//! ([`crate::checks`]), where cfMesh uses OpenFOAM's area-weighted face
//! centre.
//!
//! Deterministic: every loop runs in index order and positions are updated
//! Jacobi-style (all new positions from the old ones), as upstream's
//! `updateVerticesSMP`.

use crate::checks::cell_geometry;
use crate::math::Vec3;
use crate::volume_mesh::VolumeMesh;
use petir::mathf::RealMath;

/// OpenFOAM's `SMALL` for `double`.
const SMALL: f64 = 1e-15;
/// OpenFOAM's `VSMALL` for `double`.
const VSMALL: f64 = 1e-300;

/// `partTetMesh::vertexTypes`.
const NONE: u8 = 0;
const SMOOTH: u8 = 1;
const BOUNDARY: u8 = 2;
const FACECENTRE: u8 = 4;
const CELLCENTRE: u8 = 8;
const LOCKED: u8 = 16;

/// Signed volume of the tetrahedron `(a, b, c, d)`: OpenFOAM's
/// `tetrahedron::mag`, `((b − a) × (c − a)) · (d − a) / 6`.
fn tet_volume(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f64 {
    b.sub(a).cross(c.sub(a)).dot(d.sub(a)) / 6.0
}

/// For each point, the cells using it, ascending (`pointCells`).
pub(crate) fn point_cells(mesh: &VolumeMesh) -> Vec<Vec<usize>> {
    let mut pc: Vec<Vec<usize>> = vec![Vec::new(); mesh.points.len()];
    for f in 0..mesh.face_count() {
        for c in std::iter::once(mesh.owner[f]).chain(mesh.neighbour[f]) {
            for &p in &mesh.faces[f] {
                pc[p].push(c);
            }
        }
    }
    for v in pc.iter_mut() {
        v.sort_unstable();
        v.dedup();
    }
    pc
}

/// For each cell, its faces, ascending.
pub(crate) fn cell_faces(mesh: &VolumeMesh) -> Vec<Vec<usize>> {
    let mut cf: Vec<Vec<usize>> = vec![Vec::new(); mesh.n_cells];
    for f in 0..mesh.face_count() {
        cf[mesh.owner[f]].push(f);
        if let Some(n) = mesh.neighbour[f] {
            cf[n].push(f);
        }
    }
    cf
}

/// The cells around a set of faces decomposed into tetrahedra
/// (`partTetMesh`).
#[derive(Debug, Clone)]
pub struct PartTetMesh {
    /// Mesh points in the region, then face centres, then cell centres.
    points: Vec<Vec3>,
    /// Tetrahedra as point indices; positive volume when valid.
    tets: Vec<[usize; 4]>,
    /// For each node, the mesh point it stands for (`usize::MAX` for a
    /// face or cell centre).
    node_in_orig: Vec<usize>,
    /// `partTetMesh` vertex type flags.
    kind: Vec<u8>,
    /// Tetrahedra around each node.
    point_tets: Vec<Vec<usize>>,
}

impl PartTetMesh {
    /// `partTetMesh(mesh, lockedPoints, badFaces, additionalLayers)`: every
    /// cell touching a point of a face in `faces`, grown `additional_layers`
    /// times through shared points.
    pub fn around_faces(mesh: &VolumeMesh, locked: &[bool], faces: &[bool], additional_layers: usize) -> Self {
        let pc = point_cells(mesh);
        let cf = cell_faces(mesh);
        let mut use_cell = vec![0usize; mesh.n_cells];
        for (f, &bad) in faces.iter().enumerate() {
            if bad {
                for &p in &mesh.faces[f] {
                    for &c in &pc[p] {
                        use_cell[c] = 1;
                    }
                }
            }
        }
        for layer in 1..=additional_layers {
            for c in 0..mesh.n_cells {
                if use_cell[c] == layer {
                    for &f in &cf[c] {
                        for &p in &mesh.faces[f] {
                            for &cl in &pc[p] {
                                if use_cell[cl] == 0 {
                                    use_cell[cl] = layer + 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        let used: Vec<bool> = use_cell.iter().map(|&u| u > 0).collect();
        Self::from_cells(mesh, &used, locked)
    }

    /// `partTetMesh::createPointsAndTets`.
    pub fn from_cells(mesh: &VolumeMesh, use_cell: &[bool], locked: &[bool]) -> Self {
        let (_, centres) = cell_geometry(mesh);
        let nf = mesh.face_count();
        let mut used_face = vec![0u8; nf];
        for f in 0..nf {
            if use_cell[mesh.owner[f]] {
                used_face[f] += 1;
            }
            if let Some(n) = mesh.neighbour[f] {
                if use_cell[n] {
                    used_face[f] += 1;
                }
            }
        }
        let mut points: Vec<Vec3> = Vec::new();
        let mut kind: Vec<u8> = Vec::new();
        let mut node_for_point = vec![usize::MAX; mesh.points.len()];
        let mut node_for_face = vec![usize::MAX; nf];
        // Boundary faces first (their points are BOUNDARY), then internal.
        for f in (0..nf).filter(|&f| mesh.neighbour[f].is_none()) {
            if used_face[f] == 0 {
                continue;
            }
            let ring = &mesh.faces[f];
            if ring.len() > 3 {
                node_for_face[f] = points.len();
                points.push(mesh.face_centroid(f));
                kind.push(FACECENTRE);
            }
            for &p in ring {
                if node_for_point[p] == usize::MAX {
                    node_for_point[p] = points.len();
                    points.push(mesh.points[p]);
                    kind.push(BOUNDARY);
                }
            }
        }
        for f in (0..nf).filter(|&f| mesh.neighbour[f].is_some()) {
            if used_face[f] == 0 {
                continue;
            }
            let ring = &mesh.faces[f];
            if ring.len() > 3 {
                node_for_face[f] = points.len();
                points.push(mesh.face_centroid(f));
                kind.push(FACECENTRE);
            }
            let v_type = if used_face[f] == 2 { SMOOTH } else { NONE };
            for &p in ring {
                if node_for_point[p] == usize::MAX {
                    node_for_point[p] = points.len();
                    points.push(mesh.points[p]);
                    kind.push(v_type);
                } else if v_type == NONE {
                    kind[node_for_point[p]] = NONE;
                }
            }
        }
        // Tetrahedra: each used cell about its centre, each face about its
        // centre (triangles directly).
        let cf = cell_faces(mesh);
        let mut tets: Vec<[usize; 4]> = Vec::new();
        for c in 0..mesh.n_cells {
            if !use_cell[c] {
                continue;
            }
            let centre = points.len();
            points.push(centres[c]);
            kind.push(CELLCENTRE);
            for &f in &cf[c] {
                let ring = &mesh.faces[f];
                let k = ring.len();
                let n = |p: usize| node_for_point[p];
                if mesh.owner[f] == c {
                    if k == 3 {
                        tets.push([n(ring[0]), n(ring[2]), n(ring[1]), centre]);
                    } else {
                        for i in 0..k {
                            tets.push([n(ring[i]), n(ring[(i + k - 1) % k]), node_for_face[f], centre]);
                        }
                    }
                } else if k == 3 {
                    tets.push([n(ring[0]), n(ring[1]), n(ring[2]), centre]);
                } else {
                    for i in 0..k {
                        tets.push([n(ring[i]), n(ring[(i + 1) % k]), node_for_face[f], centre]);
                    }
                }
            }
        }
        let mut node_in_orig = vec![usize::MAX; points.len()];
        for (p, &node) in node_for_point.iter().enumerate() {
            if node != usize::MAX {
                if locked[p] {
                    kind[node] |= LOCKED;
                }
                node_in_orig[node] = p;
            }
        }
        let mut point_tets: Vec<Vec<usize>> = vec![Vec::new(); points.len()];
        for (t, tet) in tets.iter().enumerate() {
            for &v in tet {
                point_tets[v].push(t);
            }
        }
        PartTetMesh { points, tets, node_in_orig, kind, point_tets }
    }

    /// Number of nodes the smoothers may move.
    pub fn n_smooth_nodes(&self) -> usize {
        self.kind.iter().filter(|&&k| k & SMOOTH != 0 && k & LOCKED == 0).count()
    }

    /// `partTetMeshSimplex`: the tetrahedra around `node`, each reordered so
    /// that `node` is the fourth point, over a local copy of their points.
    /// Returns (points, tets, local index of `node`).
    fn simplex(&self, node: usize) -> (Vec<Vec3>, Vec<[usize; 4]>, usize) {
        let mut pts: Vec<Vec3> = Vec::new();
        let mut addr: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
        let mut tets = Vec::with_capacity(self.point_tets[node].len());
        for &t in &self.point_tets[node] {
            let tet = self.tets[t];
            for &v in &tet {
                addr.entry(v).or_insert_with(|| {
                    pts.push(self.points[v]);
                    pts.len() - 1
                });
            }
            let a = |v: usize| addr[&v];
            let pos = tet.iter().position(|&v| v == node).unwrap();
            tets.push(match pos {
                0 => [a(tet[1]), a(tet[3]), a(tet[2]), a(tet[0])],
                1 => [a(tet[0]), a(tet[2]), a(tet[3]), a(tet[1])],
                2 => [a(tet[0]), a(tet[3]), a(tet[1]), a(tet[2])],
                _ => [a(tet[0]), a(tet[1]), a(tet[2]), a(tet[3])],
            });
        }
        let centre = addr[&node];
        (pts, tets, centre)
    }

    /// `updateVerticesSMP`: set the new positions, then recompute every face
    /// and cell centre touching a moved node (area- and volume-weighted
    /// centroids of their tetrahedra).
    fn update_vertices(&mut self, moved: &[(usize, Vec3)]) {
        let mut update = vec![0u8; self.points.len()];
        for &(node, p) in moved {
            self.points[node] = p;
            for &t in &self.point_tets[node] {
                let tet = self.tets[t];
                if self.kind[tet[3]] & CELLCENTRE != 0 {
                    update[tet[3]] |= CELLCENTRE;
                }
                if self.kind[tet[2]] & FACECENTRE != 0 {
                    update[tet[2]] |= FACECENTRE;
                }
            }
        }
        for node in 0..self.points.len() {
            if update[node] & CELLCENTRE != 0 {
                let (mut c, mut vol) = (Vec3::ZERO, 0.0);
                for &t in &self.point_tets[node] {
                    let [a, b, cc, d] = self.tets[t].map(|v| self.points[v]);
                    let v = tet_volume(a, b, cc, d).abs() + VSMALL;
                    c = c.add(a.add(b).add(cc).add(d).scale(0.25 * v));
                    vol += v;
                }
                self.points[node] = c.scale(1.0 / vol);
            } else if update[node] & FACECENTRE != 0 {
                let (mut c, mut area) = (Vec3::ZERO, 0.0);
                for &t in &self.point_tets[node] {
                    let [a, b, cc, _] = self.tets[t].map(|v| self.points[v]);
                    let ar = b.sub(a).cross(cc.sub(a)).length() * 0.5 + VSMALL;
                    c = c.add(a.add(b).add(cc).scale(ar / 3.0));
                    area += ar;
                }
                self.points[node] = c.scale(1.0 / area);
            }
        }
    }

    /// `updateOrigMesh`: write the nodes back to the mesh; returns the faces
    /// of every cell around a moved point (`changedFace`).
    fn update_orig(&self, mesh: &mut VolumeMesh, pc: &[Vec<usize>], cf: &[Vec<usize>]) -> Vec<bool> {
        let mut changed_point = vec![false; mesh.points.len()];
        for (node, &p) in self.node_in_orig.iter().enumerate() {
            if p != usize::MAX {
                if mesh.points[p] != self.points[node] {
                    changed_point[p] = true;
                }
                mesh.points[p] = self.points[node];
            }
        }
        let mut changed_face = vec![false; mesh.face_count()];
        for (p, &ch) in changed_point.iter().enumerate() {
            if ch {
                for &c in &pc[p] {
                    for &f in &cf[c] {
                        changed_face[f] = true;
                    }
                }
            }
        }
        changed_face
    }

    /// `tetMeshOptimisation::optimiseUsingVolumeOptimizer(nIterations)`.
    pub fn optimise_using_volume_optimizer(&mut self, n_iterations: usize) {
        for _ in 0..n_iterations {
            let mut moved = Vec::new();
            for node in 0..self.points.len() {
                if self.kind[node] & LOCKED != 0 || self.kind[node] & SMOOTH == 0 {
                    continue;
                }
                let (mut pts, tets, c) = self.simplex(node);
                volume_optimizer(&mut pts, &tets, c, 1e-5);
                moved.push((node, pts[c]));
            }
            self.update_vertices(&moved);
        }
    }

    /// `tetMeshOptimisation::optimiseUsingKnuppMetric(nIterations)`: move the
    /// points of inverted tetrahedra until none is left or the count stops
    /// falling.
    pub fn optimise_using_knupp_metric(&mut self, n_iterations: usize) {
        let mut inverted: Vec<bool> = self.tets.iter().map(|t| self.tet_vol(t) < VSMALL).collect();
        let mut n_iter = 0usize;
        loop {
            let mut negative_node = vec![false; self.points.len()];
            let mut n_negative = 0usize;
            for (t, &inv) in inverted.iter().enumerate() {
                if inv {
                    n_negative += 1;
                    for &v in &self.tets[t] {
                        negative_node[v] = true;
                    }
                }
            }
            if n_negative == 0 {
                return;
            }
            let mut moved = Vec::new();
            for node in 0..self.points.len() {
                if !negative_node[node] || self.kind[node] & LOCKED != 0 || self.kind[node] & SMOOTH == 0 {
                    continue;
                }
                let (mut pts, tets, c) = self.simplex(node);
                knupp_metric(&mut pts, &tets, c);
                moved.push((node, pts[c]));
            }
            self.update_vertices(&moved);
            let before = n_negative;
            n_negative = 0;
            let mut helper = vec![false; self.tets.len()];
            for t in 0..self.tets.len() {
                if inverted[t] && self.tet_vol(&self.tets[t]) < VSMALL {
                    helper[t] = true;
                    n_negative += 1;
                }
            }
            inverted = helper;
            if n_negative == 0 {
                return;
            }
            n_iter += 1;
            if !(n_negative < before || n_iter < n_iterations) {
                return;
            }
        }
    }

    fn tet_vol(&self, t: &[usize; 4]) -> f64 {
        tet_volume(self.points[t[0]], self.points[t[1]], self.points[t[2]], self.points[t[3]])
    }
}

/// Bounding box of the simplex's opposite faces (`simplexSmoother`): the
/// first three points of every tetrahedron.
fn simplex_bb(pts: &[Vec3], tets: &[[usize; 4]]) -> (Vec3, Vec3) {
    let (mut lo, mut hi) = (Vec3::new(f64::MAX, f64::MAX, f64::MAX), Vec3::new(f64::MIN, f64::MIN, f64::MIN));
    for t in tets {
        for &v in &t[..3] {
            let p = pts[v];
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
    }
    (lo, hi)
}

fn bb_contains(lo: Vec3, hi: Vec3, p: Vec3) -> bool {
    p.x >= lo.x && p.y >= lo.y && p.z >= lo.z && p.x <= hi.x && p.y <= hi.y && p.z <= hi.z
}

/// A 3×3 matrix, rows.
type Mat3 = [[f64; 3]; 3];

fn outer(a: Vec3, b: Vec3) -> Mat3 {
    [[a.x * b.x, a.x * b.y, a.x * b.z], [a.y * b.x, a.y * b.y, a.y * b.z], [a.z * b.x, a.z * b.y, a.z * b.z]]
}

fn madd(a: Mat3, b: Mat3, s: f64) -> Mat3 {
    let mut r = a;
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] += s * b[i][j];
        }
    }
    r
}

fn det3(m: &Mat3) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// `inv(m, det) & v`.
fn solve3(m: &Mat3, det: f64, v: Vec3) -> Vec3 {
    let inv = [
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) / det,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) / det,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) / det,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) / det,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) / det,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) / det,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) / det,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) / det,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) / det,
        ],
    ];
    Vec3::new(
        inv[0][0] * v.x + inv[0][1] * v.y + inv[0][2] * v.z,
        inv[1][0] * v.x + inv[1][1] * v.y + inv[1][2] * v.z,
        inv[2][0] * v.x + inv[2][1] * v.y + inv[2][2] * v.z,
    )
}

/// `volumeOptimizer::evaluateStabilisationFactor`.
fn vo_stabilisation(pts: &[Vec3], tets: &[[usize; 4]]) -> f64 {
    let (mut v_min, mut lsq_max) = (f64::MAX, 0.0f64);
    for t in tets {
        let [a, b, c, d] = t.map(|v| pts[v]);
        v_min = v_min.min(tet_volume(a, b, c, d));
        let lsq = d.sub(a).dot(d.sub(a)) + d.sub(b).dot(d.sub(b)) + d.sub(c).dot(d.sub(c));
        lsq_max = lsq_max.max(lsq);
    }
    if v_min < SMALL * lsq_max {
        SMALL * lsq_max
    } else {
        0.0
    }
}

/// `volumeOptimizer::evaluateFunc`: Σ |L|² / V_s^(2/3) over the simplex, with
/// the stabilised volume V_s = (V + √(V² + K)) / 2.
fn vo_func(pts: &[Vec3], tets: &[[usize; 4]]) -> f64 {
    let k = vo_stabilisation(pts, tets);
    let mut func = 0.0;
    for t in tets {
        let [a, b, c, d] = t.map(|v| pts[v]);
        let lsq = d.sub(a).dot(d.sub(a)) + d.sub(b).dot(d.sub(b)) + d.sub(c).dot(d.sub(c));
        let v = tet_volume(a, b, c, d);
        let stab = (v * v + k).sqrt();
        let vs = 0.5 * (v + stab);
        func += lsq / vs.r_powf(2.0 / 3.0);
    }
    func
}

/// `volumeOptimizer::evaluateGradientsExact` (gradient and Hessian of
/// [`vo_func`] at the free point).
fn vo_gradients(pts: &[Vec3], tets: &[[usize; 4]], free: usize) -> Option<(Vec3, Mat3)> {
    let k = vo_stabilisation(pts, tets);
    let p = pts[free];
    let c23 = 2.0 / 3.0 * 0.5f64.r_powf(2.0 / 3.0);
    let c1 = c23 / 3.0;
    let mut grad = Vec3::ZERO;
    let mut hess: Mat3 = [[0.0; 3]; 3];
    for t in tets {
        let [a, b, c, d] = t.map(|v| pts[v]);
        let grad_v = b.sub(a).cross(c.sub(a)).scale(1.0 / 6.0);
        let lsq = d.sub(a).dot(d.sub(a)) + d.sub(b).dot(d.sub(b)) + d.sub(c).dot(d.sub(c));
        let v = tet_volume(a, b, c, d);
        let stab = (v * v + k).sqrt();
        let vs = 0.5 * (v + stab);
        if vs < VSMALL {
            return None; // upstream: FatalError "I cannot continue"
        }
        let grad_stab = grad_v.scale(v / stab);
        let grad_lsq = p.scale(3.0).sub(a).sub(b).sub(c).scale(2.0);
        let grad_vs = grad_v.add(grad_stab).scale(0.5);
        let vs13 = (2.0 * vs).r_powf(1.0 / 3.0);
        let vstab = vs.r_powf(2.0 / 3.0);
        let sqr_vstab = vstab * vstab;
        let grad_vstab = grad_vs.scale(2.0 * c23 / vs13);
        grad = grad.add(grad_lsq.scale(1.0 / vstab)).sub(grad_vstab.scale(lsq / sqr_vstab));
        let gvgv = outer(grad_v, grad_v);
        let grad_grad_stab = madd(gvgv.map(|r| r.map(|x| x / stab)), gvgv, -(v * v) / stab.r_powi(3));
        let grad_grad_vstab = madd(grad_grad_stab.map(|r| r.map(|x| x * c23 / vs13)), outer(grad_vs, grad_vs), -c1 * 4.0 / vs13.r_powi(4));
        // gradGradLsq = 6 I
        let mut h = hess;
        for i in 0..3 {
            h[i][i] += 6.0 / vstab;
        }
        // − twoSymm(gradLsq ⊗ gradVstab / sqrVstab)
        let gl_gv = outer(grad_lsq, grad_vstab.scale(1.0 / sqr_vstab));
        for i in 0..3 {
            for j in 0..3 {
                h[i][j] -= gl_gv[i][j] + gl_gv[j][i];
            }
        }
        h = madd(h, grad_grad_vstab, -lsq / sqr_vstab);
        h = madd(h, outer(grad_vstab, grad_vstab), 2.0 * lsq / (sqr_vstab * vstab));
        hess = h;
    }
    Some((grad, hess))
}

/// The eight corner directions of `volumeOptimizer::dirVecs`.
const DIR_VECS: [[f64; 3]; 8] = [
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [-1.0, 1.0, 1.0],
    [1.0, 1.0, 1.0],
];

/// `volumeOptimizer::optimizeNodePosition(tol)`: move `pts[free]` to improve
/// the simplex's tetrahedra. Divide and conquer over the bounding box of the
/// opposite faces (from its centre, halving the search box each pass), then
/// Newton steps with a halving line search; the Newton result is kept only if
/// it is no worse.
pub(crate) fn volume_optimizer(pts: &mut [Vec3], tets: &[[usize; 4]], free: usize, tol: f64) {
    let (mut lo, mut hi) = simplex_bb(pts, tets);
    if !bb_contains(lo, hi, pts[free]) {
        pts[free] = lo.add(hi).scale(0.5);
    }
    let scale = 1.0 / hi.sub(lo).length();
    for p in pts.iter_mut() {
        *p = p.scale(scale);
    }
    lo = lo.scale(scale);
    hi = hi.scale(scale);

    // optimiseDivideAndConquer
    let mut centre = lo.add(hi).scale(0.5);
    pts[free] = centre;
    let (mut dx, mut dy, mut dz) = ((hi.x - lo.x) / 2.0, (hi.y - lo.y) / 2.0, (hi.z - lo.z) / 2.0);
    let mut func_after = vo_func(pts, tets);
    for _ in 0..100 {
        let func_before = func_after;
        func_after = f64::MAX;
        let mut min_centre = Vec3::ZERO;
        for dv in DIR_VECS {
            pts[free] = Vec3::new(centre.x + 0.5 * dv[0] * dx, centre.y + 0.5 * dv[1] * dy, centre.z + 0.5 * dv[2] * dz);
            let f = vo_func(pts, tets);
            if f < func_after {
                min_centre = pts[free];
                func_after = f;
            }
        }
        centre = min_centre;
        pts[free] = min_centre;
        dx *= 0.5;
        dy *= 0.5;
        dz *= 0.5;
        if (func_after - func_before).abs() / func_after < tol {
            break;
        }
    }
    let func_dc = func_after;
    let copy_p = pts[free];

    // optimiseSteepestDescent (Newton with a line search)
    let mut func_after = vo_func(pts, tets);
    for _ in 0..100 {
        let p_orig = pts[free];
        let func_before = func_after;
        let mut finished = false;
        let Some((grad, hess)) = vo_gradients(pts, tets, free) else { break };
        let det = det3(&hess);
        if det > SMALL {
            let disp = solve3(&hess, det, grad);
            pts[free] = p_orig.sub(disp);
            func_after = vo_func(pts, tets);
            let mut relax = 0.8;
            let mut n_loops = 0;
            while func_after > func_before {
                pts[free] = p_orig.sub(disp.scale(relax));
                relax *= 0.5;
                func_after = vo_func(pts, tets);
                if func_after < func_before {
                    continue;
                }
                n_loops += 1;
                if n_loops == 5 {
                    pts[free] = p_orig;
                    finished = true;
                    func_after = func_before;
                }
            }
            if (func_before - func_after).abs() / func_before < tol {
                finished = true;
            }
        } else {
            // move off a zero volume along the normals of the flat tets
            let mut disp = Vec3::ZERO;
            for t in tets {
                let [a, b, c, d] = t.map(|v| pts[v]);
                if tet_volume(a, b, c, d) < SMALL {
                    let n = b.sub(a).cross(c.sub(a));
                    let m = n.length();
                    if m > VSMALL {
                        disp = disp.add(n.scale(0.01 / m));
                    }
                }
            }
            pts[free] = pts[free].add(disp);
            func_after = vo_func(pts, tets);
        }
        if finished {
            break;
        }
    }
    if func_after > func_dc {
        pts[free] = copy_p;
    }
    for p in pts.iter_mut() {
        *p = p.scale(1.0 / scale);
    }
}

/// `knuppMetric::optimizeNodePosition`: Newton on Σ (|f| − f)², f = n̂·(p −
/// c) − β over the opposite faces, β = 0.01 × the bounding-box diagonal,
/// halving β up to five times while the metric reaches zero without β but not
/// with it.
pub(crate) fn knupp_metric(pts: &mut [Vec3], tets: &[[usize; 4]], free: usize) {
    let (lo, hi) = simplex_bb(pts, tets);
    let mut normals = Vec::new();
    let mut centres = Vec::new();
    for t in tets {
        let [a, b, c, _] = t.map(|v| pts[v]);
        let n = b.sub(a).cross(c.sub(a)).scale(0.5);
        let d = n.length();
        if d > VSMALL {
            centres.push(a.add(b).add(c).scale(1.0 / 3.0));
            normals.push(n.scale(1.0 / d));
        }
    }
    let mut beta = 0.01 * hi.sub(lo).length();
    let mut p = pts[free];
    if !bb_contains(lo, hi, p) {
        p = lo.add(hi).scale(0.5);
    }
    let tol = (2.0 * SMALL) * (2.0 * SMALL) * hi.sub(lo).dot(hi.sub(lo));
    let metric = |p: Vec3, beta: f64| -> f64 {
        normals.iter().zip(&centres).map(|(n, c)| {
            let fx = n.dot(p.sub(*c)) - beta;
            (fx.abs() - fx) * (fx.abs() - fx)
        }).sum::<f64>()
    };
    let sign = |x: f64| if x >= 0.0 { 1.0 } else { -1.0 };
    for _outer in 0..5 {
        let mut finished = true;
        let mut last = metric(p, beta);
        for _ in 0..10 {
            let p_orig = p;
            let (mut g, mut h) = (Vec3::ZERO, [[0.0; 3]; 3]);
            for (n, c) in normals.iter().zip(&centres) {
                let fx = n.dot(p.sub(*c)) - beta;
                let gfx = n.scale(sign(fx) - 1.0);
                g = g.add(gfx.scale(fx.abs() - fx));
                h = madd(h, outer(gfx, gfx), 1.0);
            }
            let det = det3(&h);
            let mut disp = Vec3::ZERO;
            if det > SMALL {
                disp = solve3(&h, det, g);
                if !(disp.x.is_finite() && disp.y.is_finite() && disp.z.is_finite()) {
                    disp = Vec3::ZERO;
                }
                p = p.sub(disp);
                let mut func = metric(p, beta);
                let mut relax = 0.8;
                let mut n_loops = 0;
                while func > last {
                    p = p_orig.sub(disp.scale(relax));
                    relax *= 0.5;
                    func = metric(p, beta);
                    if func < last {
                        continue;
                    }
                    n_loops += 1;
                    if n_loops == 5 {
                        p = p_orig;
                        disp = Vec3::ZERO;
                        func = 0.0;
                    }
                }
                last = func;
            }
            if disp.dot(disp) <= tol {
                break;
            }
        }
        if last < VSMALL && metric(p, 0.0) > VSMALL {
            beta /= 2.0;
            finished = false;
        }
        if finished {
            break;
        }
    }
    pts[free] = p;
}

/// cfMesh's checks on the faces flagged `active`; returns the faces failing.
/// `low_quality` adds `checkFaceDotProduct(65°)` and
/// `checkFaceSkewness(2.0)` (`findLowQualityFaces`); otherwise it is
/// `findBadFaces` (pyramids, flatness 0.8, cell-part tetrahedra, areas) or,
/// with `relaxed`, `findBadFacesRelaxed` (pyramids and areas only).
fn flag_faces(mesh: &VolumeMesh, active: &[bool], mode: CheckMode) -> Vec<bool> {
    let (_, centres) = cell_geometry(mesh);
    let mut bad = vec![false; mesh.face_count()];
    let cos65 = 65.0f64.to_radians().r_cos();
    for f in 0..mesh.face_count() {
        if !active[f] {
            continue;
        }
        let s = mesh.face_area_vector(f);
        let fc = mesh.face_centroid(f);
        let co = centres[mesh.owner[f]];
        let ring = &mesh.faces[f];
        let k = ring.len();
        let mut b = false;
        if mode == CheckMode::LowQuality {
            if let Some(n) = mesh.neighbour[f] {
                // checkFaceDotProduct: internal faces.
                let d = centres[n].sub(co);
                let dots = d.dot(s) / (d.length() * s.length() + VSMALL);
                if dots < cos65 {
                    b = true;
                }
                // checkFaceSkewness (cfMesh's): internal.
                let cn = centres[n];
                let d_own = fc.sub(co).length();
                let d_nei = fc.sub(cn).length();
                let x = co.scale(d_nei / (d_own + d_nei)).add(cn.scale(d_own / (d_own + d_nei)));
                if fc.sub(x).length() / (cn.sub(co).length() + VSMALL) > 2.0 {
                    b = true;
                }
            } else {
                let d = fc.sub(co);
                let m = s.length();
                if m > VSMALL {
                    let n = s.scale(1.0 / m);
                    let dn = n.scale(n.dot(d));
                    if d.sub(dn).length() / (d.length() + VSMALL) > 2.0 {
                        b = true;
                    }
                }
            }
        } else {
            // checkFacePyramids(VSMALL)
            if fc.sub(co).dot(s) / 3.0 < VSMALL {
                b = true;
            }
            if let Some(n) = mesh.neighbour[f] {
                if centres[n].sub(fc).dot(s) / 3.0 < VSMALL {
                    b = true;
                }
            }
            // checkFaceAreas(VSMALL)
            if s.length() < VSMALL {
                b = true;
            }
            if mode == CheckMode::Bad {
                // checkFaceFlatness(0.8)
                if k > 3 && s.length() > VSMALL {
                    let mut sum_a = 0.0;
                    for i in 0..k {
                        let p = mesh.points[ring[i]];
                        let q = mesh.points[ring[(i + 1) % k]];
                        sum_a += q.sub(p).cross(fc.sub(p)).length() * 0.5;
                    }
                    if s.length() / (sum_a + VSMALL) < 0.8 {
                        b = true;
                    }
                }
                // checkCellPartTetrahedra(VSMALL)
                for i in 0..k {
                    let p = mesh.points[ring[i]];
                    let q = mesh.points[ring[(i + 1) % k]];
                    if tet_volume(fc, q, p, co) < VSMALL {
                        b = true;
                    }
                    if let Some(n) = mesh.neighbour[f] {
                        if tet_volume(fc, p, q, centres[n]) < VSMALL {
                            b = true;
                        }
                    }
                }
            }
        }
        bad[f] = b;
    }
    bad
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckMode {
    Bad,
    Relaxed,
    LowQuality,
}

/// `polyMeshGenChecks::findBadFaces` on every face (see the module docs).
pub fn find_bad_faces(mesh: &VolumeMesh) -> Vec<bool> {
    flag_faces(mesh, &vec![true; mesh.face_count()], CheckMode::Bad)
}

/// `polyMeshGenChecks::findLowQualityFaces` on every face: non-orthogonality
/// above 65° or cfMesh skewness above 2.
pub fn find_low_quality_faces(mesh: &VolumeMesh) -> Vec<bool> {
    flag_faces(mesh, &vec![true; mesh.face_count()], CheckMode::LowQuality)
}

/// What one optimiser call did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OptimiserReport {
    /// Faces flagged at the first check.
    pub flagged_before: usize,
    /// Faces flagged at the last check.
    pub flagged_after: usize,
    /// Inner iterations run.
    pub iterations: usize,
}

/// `meshOptimizer::optimizeLowQualityFaces(maxNumIterations)`: repeatedly
/// flag the low-quality faces among those changed by the last pass (all, at
/// first) and run the volume optimiser on the cells two point-layers around
/// them. `locked` points never move (cfMesh `LOCKED`); boundary points never
/// move.
pub fn optimize_low_quality_faces(mesh: &mut VolumeMesh, locked: &[bool], max_iterations: usize) -> OptimiserReport {
    let pc = point_cells(mesh);
    let cf = cell_faces(mesh);
    let mut changed = vec![true; mesh.face_count()];
    let mut rep = OptimiserReport::default();
    for it in 0..max_iterations {
        let low = flag_faces(mesh, &changed, CheckMode::LowQuality);
        let n_bad = low.iter().filter(|&&b| b).count();
        if it == 0 {
            rep.flagged_before = n_bad;
        }
        rep.flagged_after = n_bad;
        rep.iterations = it + 1;
        if n_bad == 0 {
            break;
        }
        let mut tm = PartTetMesh::around_faces(mesh, locked, &low, 2);
        tm.optimise_using_volume_optimizer(10);
        changed = tm.update_orig(mesh, &pc, &cf);
    }
    rep
}

/// `meshOptimizer::untangleMeshFV(maxNumGlobalIterations, maxNumIterations,
/// maxNumSurfaceIterations, relaxedCheck)`, interior part: flag the bad faces
/// among those changed (all, at first), build the region around them grown by
/// `nGlobalIter / 2 + 1` layers, and run the Knupp untangler then the volume
/// optimiser; stop when no bad face is left, or five passes after the fewest
/// bad faces, or after `max_iterations`; repeat up to
/// `max_global_iterations` times. The LP mesh untangler and the boundary
/// moves are not ported (module docs), so `_max_surface_iterations` is
/// accepted and unused.
pub fn untangle_mesh_fv(
    mesh: &mut VolumeMesh,
    locked: &[bool],
    max_global_iterations: usize,
    max_iterations: usize,
    _max_surface_iterations: usize,
    relaxed: bool,
) -> OptimiserReport {
    let pc = point_cells(mesh);
    let cf = cell_faces(mesh);
    let mut changed = vec![true; mesh.face_count()];
    let mode = if relaxed { CheckMode::Relaxed } else { CheckMode::Bad };
    let mut rep = OptimiserReport::default();
    let mut first = true;
    let mut n_global = 0usize;
    loop {
        let mut n_iter = 0usize;
        let (mut min_bad, mut min_iter) = (usize::MAX, 0usize);
        let mut n_bad;
        loop {
            let bad = flag_faces(mesh, &changed, mode);
            n_bad = bad.iter().filter(|&&b| b).count();
            if first {
                rep.flagged_before = n_bad;
                first = false;
            }
            rep.flagged_after = n_bad;
            if n_bad == 0 {
                break;
            }
            if n_bad < min_bad {
                min_bad = n_bad;
                min_iter = n_iter;
            }
            let mut tm = PartTetMesh::around_faces(mesh, locked, &bad, n_global / 2 + 1);
            tm.optimise_using_knupp_metric(5);
            tm.optimise_using_volume_optimizer(10);
            changed = tm.update_orig(mesh, &pc, &cf);
            rep.iterations += 1;
            n_iter += 1;
            if !(n_iter < min_iter + 5 + 1 && n_iter < max_iterations) {
                break;
            }
        }
        n_global += 1;
        if n_bad == 0 || n_global >= max_global_iterations {
            break;
        }
    }
    rep
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;
    use crate::checks::check_quality;

    /// V&V (gh:#1215) — the volume optimiser on a single regular simplex.
    /// Methodology: the eight tetrahedra around the centre node of a 2×2×2
    /// Cartesian block's octahedral star (the node and its six axis
    /// neighbours, faces of the octahedron as opposite faces); the node is
    /// displaced by 0.3 along x. Prediction (written before the run): the
    /// functional Σ|L|²/V^(2/3) is symmetric about the octahedron's centre, so
    /// the optimiser returns the node to the centre to 1e-4 (the divide and
    /// conquer stops at a relative change of 1e-5; Newton then refines).
    #[test]
    fn volume_optimizer_recentres_a_regular_star() {
        let ax = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, -1.0)];
        let mut pts: Vec<Vec3> = ax.to_vec();
        pts.push(Vec3::new(0.3, 0.0, 0.0));
        let free = 6;
        // The eight octants: (±x, ±y, ±z), wound so the free point sits on
        // the positive side of each opposite face.
        let mut tets = Vec::new();
        for &(i, j, k) in &[(0, 2, 4), (2, 1, 4), (1, 3, 4), (3, 0, 4), (2, 0, 5), (1, 2, 5), (3, 1, 5), (0, 3, 5)] {
            let mut t = [i, j, k, free];
            if tet_volume(pts[t[0]], pts[t[1]], pts[t[2]], Vec3::ZERO) < 0.0 {
                t.swap(0, 1);
            }
            tets.push(t);
        }
        volume_optimizer(&mut pts, &tets, free, 1e-5);
        assert!(pts[free].length() < 1e-4, "recentred: {:?}", pts[free]);
    }

    /// V&V (gh:#1215) — the Knupp untangler on an inverted star. Same star,
    /// the node pushed outside it to (1.4, 0, 0) so four tetrahedra are
    /// inverted. Prediction: after `knupp_metric` every tetrahedron of the
    /// star has positive volume (the metric is zero only when the point is
    /// at least β inside every opposite face).
    #[test]
    fn knupp_metric_untangles_an_inverted_star() {
        let ax = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, -1.0)];
        let mut pts: Vec<Vec3> = ax.to_vec();
        pts.push(Vec3::ZERO);
        let free = 6;
        let mut tets = Vec::new();
        for &(i, j, k) in &[(0, 2, 4), (2, 1, 4), (1, 3, 4), (3, 0, 4), (2, 0, 5), (1, 2, 5), (3, 1, 5), (0, 3, 5)] {
            let mut t = [i, j, k, free];
            if tet_volume(pts[t[0]], pts[t[1]], pts[t[2]], pts[free]) < 0.0 {
                t.swap(0, 1);
            }
            tets.push(t);
        }
        pts[free] = Vec3::new(1.4, 0.0, 0.0);
        let inverted = tets.iter().filter(|t| tet_volume(pts[t[0]], pts[t[1]], pts[t[2]], pts[t[3]]) <= 0.0).count();
        assert_eq!(inverted, 4);
        knupp_metric(&mut pts, &tets, free);
        for t in &tets {
            assert!(tet_volume(pts[t[0]], pts[t[1]], pts[t[2]], pts[t[3]]) > 0.0, "{:?}", pts[free]);
        }
    }

    /// V&V (gh:#1215) — **a good mesh is left alone.** Methodology: a 4×4×4
    /// Cartesian unit box (orthogonal, skew 0). Prediction: no bad and no
    /// low-quality face, so both optimisers return at once and every point
    /// stays where it was, bit for bit.
    #[test]
    fn optimisers_leave_a_good_mesh_untouched() {
        let m0 = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [4, 4, 4]);
        let mut m = m0.clone();
        let locked = vec![false; m.points.len()];
        assert_eq!(find_bad_faces(&m).iter().filter(|&&b| b).count(), 0);
        assert_eq!(find_low_quality_faces(&m).iter().filter(|&&b| b).count(), 0);
        let r1 = optimize_low_quality_faces(&mut m, &locked, 10);
        let r2 = untangle_mesh_fv(&mut m, &locked, 10, 50, 2, false);
        assert_eq!((r1.flagged_before, r2.flagged_before), (0, 0));
        assert_eq!(m.points, m0.points);
    }

    /// V&V (gh:#1215) — **a dented mesh is repaired, the boundary is not
    /// moved.** Methodology: the 4×4×4 Cartesian box with its interior point
    /// (0.5, 0.5, 0.5) pushed to (0.5, 0.5, 0.9), past the next point layer (z = 0.75) and the neighbouring
    /// layer's mid-plane, which inverts face pyramids of the cells above it.
    /// Prediction: `untangle_mesh_fv` clears every bad face (the point has
    /// room to go back), leaves every boundary point bit-identical, and the
    /// result has no negative-volume cell; `optimize_low_quality_faces` then
    /// lowers the max non-orthogonality.
    #[test]
    fn untangle_repairs_a_dented_box_without_moving_the_boundary() {
        let mut m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [4, 4, 4]);
        let centre = m.points.iter().position(|p| p.sub(Vec3::new(0.5, 0.5, 0.5)).length() < 1e-12).unwrap();
        m.points[centre] = Vec3::new(0.5, 0.5, 0.9);
        let before = find_bad_faces(&m).iter().filter(|&&b| b).count();
        assert!(before > 0, "the dent makes bad faces");
        let boundary: Vec<usize> = (m.n_internal_faces()..m.face_count()).flat_map(|f| m.faces[f].clone()).collect();
        let saved: Vec<Vec3> = boundary.iter().map(|&p| m.points[p]).collect();
        let locked = vec![false; m.points.len()];
        let r = untangle_mesh_fv(&mut m, &locked, 10, 50, 2, false);
        assert_eq!(find_bad_faces(&m).iter().filter(|&&b| b).count(), 0, "{r:?}");
        assert!(boundary.iter().zip(&saved).all(|(&p, s)| m.points[p] == *s));
        let q0 = check_quality(&m).max_non_orthogonality_deg;
        let _ = optimize_low_quality_faces(&mut m, &locked, 10);
        let q = check_quality(&m);
        assert_eq!(q.n_negative_volume_cells, 0);
        assert!(q.max_non_orthogonality_deg <= q0 + 1e-9, "{q0} -> {}", q.max_non_orthogonality_deg);
        // Deterministic.
        let mut m2 = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [4, 4, 4]);
        m2.points[centre] = Vec3::new(0.5, 0.5, 0.9);
        let _ = untangle_mesh_fv(&mut m2, &locked, 10, 50, 2, false);
        let _ = optimize_low_quality_faces(&mut m2, &locked, 10);
        assert_eq!(m2.points, m.points);
    }
}
