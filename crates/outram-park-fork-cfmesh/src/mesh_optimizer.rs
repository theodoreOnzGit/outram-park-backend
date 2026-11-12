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
//   meshLibrary/utilities/smoothers/geometry/meshOptimizer/
//     meshOptimizerOptimizeSurface.C (optimizeSurface; gh:#1259)
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
//! - (gh:#1255) [`untangle_mesh_fv_with`] and [`UntangleControls`]: the
//!   boundary loop of `untangleMeshFV` ([`PartTetMesh::optimise_boundary_volume_optimizer`],
//!   [`PartTetMesh::optimise_boundary_surface_laplace`]), opt-in;
//!   [`optimize_mesh_near_boundaries`] with [`PartTetMesh::near_boundary`];
//!   [`optimize_mesh_fv_best_quality`] with [`find_worst_quality_faces`].
//! - (gh:#1256) [`laplace_smoother`] (`laplaceSmoother`: `laplacian`,
//!   `laplacianPC`, `laplacianWPC`, `laplacianSurface`) and [`MeshOptimizer`]
//!   (the lock API, `enforceConstraints`, `optimizeMeshFV`).
//! - (gh:#1260) [`PartTetMesh::optimise_using_mesh_untangler`]: the LP
//!   `meshUntangler` ([`crate::mesh_untangler`]), opt-in through
//!   [`UntangleControls::mesh_untangler`].
//!
//! ~~**Not ported, said so where it matters:** the LP-based `meshUntangler`
//! (`tmo.optimiseUsingMeshUntangler()`, the middle of the three smoothers in
//! `untangleMeshFV`), the boundary-point moves of `untangleMeshFV`'s second
//! loop (`optimiseBoundaryVolumeOptimizer`, `optimiseBoundarySurfaceLaplace`:
//! boundary points never move here),~~ **CHANGED 2026-11-05:** both are
//! ported (gh:#1260, gh:#1255) and off by default, so the recorded results
//! stand. **Not ported:** the parallel buffer layers. ~~`tetMatcher` (a
//! tetrahedral cell is decomposed like any other; this crate's tet-dual meshes
//! have none)~~ **CORRECTED 2026-11-11 (gh:#1385 round 3):** a cell of four
//! triangles is one tet with no centre node (`match_tet`; asmoOctree has 324).
//! ~~Centres are this crate's: the face centre is the vertex average and the
//! cell centre the pyramid-decomposition centroid ([`crate::checks`]), where
//! cfMesh uses OpenFOAM's area-weighted face centre.~~ **CORRECTED 2026-11-11
//! (gh:#1385 round 3):** the part tet mesh, the Laplacian smoother and the
//! quality checks (`flag_faces`) read cfMesh's polyMeshGen addressing geometry
//! ([`crate::checks::pmg_geometry`]); this module no longer uses
//! [`crate::checks::cell_geometry`] (a test still does).
//!
//! Deterministic: every loop runs in index order and positions are updated
//! Jacobi-style (all new positions from the old ones), as upstream's
//! `updateVerticesSMP`.

use crate::checks::{foam_cell_centres_and_vols, pmg_face_centre_and_area};
use crate::math::Vec3;
use crate::mesh_subsets::MeshSubsets;
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
    (1.0 / 6.0) * b.sub(a).cross(c.sub(a)).dot(d.sub(a))
}

/// `tetMatcher::matchShape(false, faces, owner, celli, cell)` for a cell of
/// four triangles on four distinct points: the mesh point labels
/// `vertLabels` (the base face `cell[0]` walked from its vertex 0 in the
/// direction its orientation gives, then the apex found on the face that
/// shares the edge `(v2, v0)`). `None` when the cell is not a tetrahedron.
fn match_tet(mesh: &VolumeMesh, c: usize, cell: &[usize]) -> Option<[usize; 4]> {
    if cell.len() != 4 || cell.iter().any(|&f| mesh.faces[f].len() != 3) {
        return None;
    }
    let mut distinct: Vec<usize> = Vec::new();
    for &f in cell {
        for &v in &mesh.faces[f] {
            if !distinct.contains(&v) {
                distinct.push(v);
            }
        }
    }
    if distinct.len() != 4 {
        return None;
    }
    // `nextVert(i, size, rightHand)`
    let next = |i: usize, right: bool| if right { (i + 1) % 3 } else { (3 + i - 1) % 3 };
    let f3 = cell[0];
    let face3 = &mesh.faces[f3];
    let right3 = mesh.owner[f3] != c;
    let i1 = next(0, right3);
    let i2 = next(i1, right3);
    let (v0, v1, v2) = (face3[0], face3[i1], face3[i2]);
    // the other face holding the edge (v2, v0): upstream's `face1`
    let f1 = *cell[1..].iter().find(|&&f| mesh.faces[f].contains(&v2) && mesh.faces[f].contains(&v0))?;
    let face1 = &mesh.faces[f1];
    let i0 = face1.iter().position(|&v| v == v0)?;
    let apex = face1[next(i0, mesh.owner[f1] == c)];
    Some([v0, v1, v2, apex])
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

/// `partTetMesh`'s "add additional layer of cells" loop, shared by its
/// constructors: cells marked `layer` mark every unmarked cell sharing a
/// point with them `layer + 1`.
fn grow_layers(mesh: &VolumeMesh, pc: &[Vec<usize>], cf: &[Vec<usize>], use_cell: &mut [usize], n_layers: usize) {
    for layer in 1..=n_layers {
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
        grow_layers(mesh, &pc, &cf, &mut use_cell, additional_layers);
        let used: Vec<bool> = use_cell.iter().map(|&u| u > 0).collect();
        Self::from_cells(mesh, &used, locked)
    }

    /// `partTetMesh(mesh, lockedPoints, nLayers)`: the owner cell of every
    /// boundary face, grown `n_layers` times through shared points (the
    /// region of `meshOptimizer::optimizeMeshNearBoundaries`).
    pub fn near_boundary(mesh: &VolumeMesh, locked: &[bool], n_layers: usize) -> Self {
        let pc = point_cells(mesh);
        let cf = cell_faces(mesh);
        let mut use_cell = vec![0usize; mesh.n_cells];
        for f in 0..mesh.face_count() {
            if mesh.neighbour[f].is_none() {
                use_cell[mesh.owner[f]] = 1;
            }
        }
        grow_layers(mesh, &pc, &cf, &mut use_cell, n_layers);
        let used: Vec<bool> = use_cell.iter().map(|&u| u > 0).collect();
        Self::from_cells(mesh, &used, locked)
    }

    /// `partTetMesh::createPointsAndTets`.
    pub fn from_cells(mesh: &VolumeMesh, use_cell: &[bool], locked: &[bool]) -> Self {
        // upstream reads `addressingData().faceCentres()` / `cellCentres()`:
        // cfMesh's own formulas (`pmg_face_centre_and_area`,
        // `foam_cell_centres_and_vols`), not the vertex average
        let (_, centres) = foam_cell_centres_and_vols(mesh);
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
                points.push(pmg_face_centre_and_area(&mesh.points, ring).0);
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
                points.push(pmg_face_centre_and_area(&mesh.points, ring).0);
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
        // the faces of a cell in cfMesh's cell-list order (`cells[cI]`)
        let cf = mesh.cell_face_lists();
        let mut tets: Vec<[usize; 4]> = Vec::new();
        for c in 0..mesh.n_cells {
            if !use_cell[c] {
                continue;
            }
            // `tetMatcher::matchShape`: a cell of four triangles is one tet,
            // with no cell-centre node
            if let Some(t) = match_tet(mesh, c, &cf[c]) {
                tets.push(t.map(|v| node_for_point[v]));
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
                self.points[node] = c.div(vol);
            } else if update[node] & FACECENTRE != 0 {
                let (mut c, mut area) = (Vec3::ZERO, 0.0);
                for &t in &self.point_tets[node] {
                    let [a, b, cc, _] = self.tets[t].map(|v| self.points[v]);
                    let ar = b.sub(a).cross(cc.sub(a)).length() * 0.5 + VSMALL;
                    // `c /= 3` then `centre += c*area`
                    c = c.add(a.add(b).add(cc).div(3.0).scale(ar));
                    area += ar;
                }
                self.points[node] = c.div(area);
            }
        }
    }

    /// `updateOrigMesh`: write the nodes back to the mesh; returns the faces
    /// of every cell around a moved point (`changedFace`).
    fn update_orig(&self, mesh: &mut VolumeMesh, pc: &[Vec<usize>], cf: &[Vec<usize>]) -> Vec<bool> {
        let mut changed_point = vec![false; mesh.points.len()];
        for (node, &p) in self.node_in_orig.iter().enumerate() {
            if p != usize::MAX {
                // upstream marks every node of the tet mesh as changed, moved
                // or not (`changedNode[nodeLabelInOrigMesh_[pI]] = true`), so
                // the next `changedFace` covers every cell around the region
                // (gh:#1385 round 3; a locked node's cells were left out)
                changed_point[p] = true;
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
        for it in 0..n_iterations {
            let nodes: Vec<usize> = (0..self.points.len()).filter(|&n| self.kind[n] & LOCKED == 0 && self.kind[n] & SMOOTH != 0).collect();
            let moved = self.smooth_nodes(&nodes, |pts, tets, c| volume_optimizer(pts, tets, c, 1e-5));
            self.update_vertices(&moved);
            if self.points.len() == 21340 && std::env::var("GH1385_TRACE").is_ok() {
                // gh:#1385: `VI iteration node x y z`, as the instrumented upstream prints it
                for (q, p) in self.points.iter().enumerate() {
                    eprintln!("VI {it} {q} {:?} {:?} {:?}", p.x, p.y, p.z);
                }
            }
        }
    }

    /// `tetMeshOptimisation::optimiseUsingMeshUntangler(nIterations)`
    /// (upstream default 5): the LP untangler ([`crate::mesh_untangler::mesh_untangler`])
    /// on every smoothable node of an inverted tetrahedron, until none is
    /// left or the count stops falling (`while ((nNegative <
    /// nNegativeBefore) || (++nIter < nIterations))`, short-circuit kept).
    ///
    /// **Upstream behaviour kept:** unlike the Knupp loop, the re-check never
    /// sets `helper[tetI] = true`, so the list of inverted tetrahedra carried
    /// to the next pass is empty and that pass returns at once: in effect one
    /// pass, whatever `nIterations` is.
    pub fn optimise_using_mesh_untangler(&mut self, n_iterations: usize) {
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
            let nodes: Vec<usize> =
                (0..self.points.len()).filter(|&n| negative_node[n] && self.kind[n] & LOCKED == 0 && self.kind[n] & SMOOTH != 0).collect();
            let moved = self.smooth_nodes(&nodes, |pts, tets, c| {
                crate::mesh_untangler::mesh_untangler(pts, tets, c);
            });
            self.update_vertices(&moved);
            let before = n_negative;
            n_negative = (0..self.tets.len()).filter(|&t| inverted[t] && self.tet_vol(&self.tets[t]) < VSMALL).count();
            if n_negative == 0 {
                return;
            }
            // upstream: invertedTets.transfer(helper), helper all false.
            inverted = vec![false; self.tets.len()];
            if n_negative < before {
                continue;
            }
            n_iter += 1;
            if n_iter >= n_iterations {
                return;
            }
        }
    }

    /// The number of tetrahedra with volume below `VSMALL` (the count the
    /// untanglers work on).
    pub fn n_inverted_tets(&self) -> usize {
        self.tets.iter().filter(|t| self.tet_vol(t) < VSMALL).count()
    }

    /// `tetMeshOptimisation::optimiseUsingKnuppMetric(nIterations)`: move the
    /// points of inverted tetrahedra with Knupp's metric.
    ///
    /// **gh:#1284 (2026-11-05): transcribed as upstream counts, which is
    /// one pass.** In DAFoam/cfmesh `e944f6ba695f` (and in wyldckat/cfMesh
    /// v1.0 and the full v1.1 tree) the loop that checks which tetrahedra
    /// were repaired sets `nNegative = 0` and never increments it: it marks
    /// `helper[tetI]` but has no `++nNegative`, unlike the same loop in
    /// `optimiseUsingMeshUntangler`. So `if (nNegative == 0) return;` after
    /// it always returns, and the `while ((nNegative < nNegativeBefore) ||
    /// (++nIter < nIterations))` condition is never reached: upstream runs
    /// **exactly one** Knupp pass whenever an inverted tetrahedron exists,
    /// whatever `nIterations` is. This looks like an upstream defect, but
    /// upstream is the specification, so the port keeps it (the count the
    /// loop would have needed is left in, named, and unused for control).
    ///
    /// ~~Port defect found 2026-11-05 (gh:#1260), not fixed: upstream's
    /// short-circuit `||` increments `nIter` only when the count did not
    /// fall; this loop increments it on every pass.~~ **CORRECTED
    /// 2026-11-05 (gh:#1284):** that reading missed the uncounted repair
    /// loop. Before gh:#1284 the port counted the repaired tetrahedra and
    /// ran up to `n_iterations` passes (more while the count fell);
    /// upstream runs one.
    pub fn optimise_using_knupp_metric(&mut self, n_iterations: usize) {
        let mut inverted: Vec<bool> = self.tets.iter().map(|t| self.tet_vol(t) < VSMALL).collect();
        let mut n_iter = 0usize;
        loop {
            // find the number of inverted tets
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
            // smooth the mesh
            let nodes: Vec<usize> =
                (0..self.points.len()).filter(|&n| negative_node[n] && self.kind[n] & LOCKED == 0 && self.kind[n] & SMOOTH != 0).collect();
            let moved = self.smooth_nodes(&nodes, knupp_metric);
            self.update_vertices(&moved);
            // check which tets have been repaired
            let n_negative_before = n_negative;
            // upstream: `nNegative = 0;` and the loop below never adds to it
            let n_negative = 0usize;
            let mut helper = vec![false; self.tets.len()];
            for t in 0..self.tets.len() {
                if inverted[t] && self.tet_vol(&self.tets[t]) < VSMALL {
                    helper[t] = true;
                }
            }
            inverted = helper;
            if n_negative == 0 {
                return;
            }
            // `while ((nNegative < nNegativeBefore) || (++nIter < nIterations))`,
            // unreachable as upstream has it
            if n_negative < n_negative_before {
                continue;
            }
            n_iter += 1;
            if n_iter >= n_iterations {
                return;
            }
        }
    }

    /// New positions of `nodes`, each from its simplex over the current
    /// points (Jacobi: no node sees another's move, as upstream's
    /// `updateVerticesSMP`), in node order. Natively the nodes are shared
    /// out over the available threads; the result does not depend on how
    /// (each position is a pure function of the snapshot).
    fn smooth_nodes(&self, nodes: &[usize], f: fn(&mut [Vec3], &[[usize; 4]], usize)) -> Vec<(usize, Vec3)> {
        let one = |node: usize| {
            let (mut pts, tets, c) = self.simplex(node);
            f(&mut pts, &tets, c);
            Some((node, pts[c]))
        };
        Self::map_nodes(nodes, one)
    }

    /// As [`Self::smooth_nodes`], for a smoother that may leave a node out
    /// of the update (upstream `continue`s before appending its
    /// `labelledPoint`: a corner in `optimiseBoundaryVolumeOptimizer(true)`,
    /// a node with no boundary edge in `optimiseBoundarySurfaceLaplace`). A
    /// node left out is not "moved to where it was": its face and cell
    /// centres are not recomputed.
    fn smooth_nodes_opt(&self, nodes: &[usize], f: fn(&mut [Vec3], &[[usize; 4]], usize) -> Option<Vec3>) -> Vec<(usize, Vec3)> {
        let one = |node: usize| {
            let (mut pts, tets, c) = self.simplex(node);
            f(&mut pts, &tets, c).map(|p| (node, p))
        };
        Self::map_nodes(nodes, one)
    }

    /// `one` over `nodes`, in node order, threaded natively.
    fn map_nodes<F: Fn(usize) -> Option<(usize, Vec3)> + Sync>(nodes: &[usize], one: F) -> Vec<(usize, Vec3)> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let n_threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(8);
            if n_threads > 1 && nodes.len() > 64 {
                let chunk = nodes.len().div_ceil(n_threads);
                let one = &one;
                return std::thread::scope(|sc| {
                    let handles: Vec<_> = nodes.chunks(chunk).map(|c| sc.spawn(move || c.iter().filter_map(|&n| one(n)).collect::<Vec<_>>())).collect();
                    handles.into_iter().flat_map(|h| h.join().expect("smoother thread")).collect()
                });
            }
        }
        nodes.iter().filter_map(|&n| one(n)).collect()
    }

    /// The movable `BOUNDARY` nodes, ascending.
    fn boundary_nodes(&self) -> Vec<usize> {
        (0..self.points.len()).filter(|&n| self.kind[n] & LOCKED == 0 && self.kind[n] & BOUNDARY != 0).collect()
    }

    /// Number of `BOUNDARY` nodes the boundary smoothers may move.
    pub fn n_boundary_nodes(&self) -> usize {
        self.boundary_nodes().len()
    }

    /// `tetMeshOptimisation::optimiseBoundaryVolumeOptimizer(nIterations,
    /// nonShrinking)`: the volume optimiser on every unlocked `BOUNDARY`
    /// node. With `non_shrinking` the move is projected onto the node's
    /// surface: the normal tensor Σ n̂n̂ᵀ of the simplex's boundary triangles
    /// (each boundary edge of the opposite faces with the node) is split by
    /// OpenFOAM's `eigenValues` / `eigenVectors`
    /// ([`outram_foam_basic_lib::primitives::eigen`]); one dominant
    /// eigenvalue is a surface point (drop the normal component), two an
    /// edge point (keep the component along the edge), else a corner (not
    /// moved). Upstream's default is 3 iterations.
    pub fn optimise_boundary_volume_optimizer(&mut self, n_iterations: usize, non_shrinking: bool) {
        for _ in 0..n_iterations {
            let nodes = self.boundary_nodes();
            let moved = if non_shrinking {
                self.smooth_nodes_opt(&nodes, boundary_volume_non_shrinking)
            } else {
                self.smooth_nodes(&nodes, |pts, tets, c| volume_optimizer(pts, tets, c, 1e-5))
            };
            self.update_vertices(&moved);
        }
    }

    /// `tetMeshOptimisation::optimiseBoundarySurfaceLaplace(nIterations)`:
    /// every unlocked `BOUNDARY` node to the mean of the centres of its
    /// simplex's boundary triangles. No constraint: corners and edge points
    /// move too, as upstream. Upstream's default is 3 iterations.
    pub fn optimise_boundary_surface_laplace(&mut self, n_iterations: usize) {
        for _ in 0..n_iterations {
            let nodes = self.boundary_nodes();
            let moved = self.smooth_nodes_opt(&nodes, boundary_surface_laplace);
            self.update_vertices(&moved);
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
    // OpenFOAM `Tensor::det`, in its order of operations
    m[0][0] * m[1][1] * m[2][2] + m[0][1] * m[1][2] * m[2][0] + m[0][2] * m[1][0] * m[2][1]
        - m[0][0] * m[1][2] * m[2][1]
        - m[0][1] * m[1][0] * m[2][2]
        - m[0][2] * m[1][1] * m[2][0]
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
/// [`vo_func`] at the free point), operation for operation as upstream's
/// expressions associate (gh:#1385 round 3): every `x/s` is a division, every
/// `s*x/t` is `(s*x)/t`, `gradF += a - b` adds the difference, and
/// `pow(stab, 3)` / `pow(Vs13, 4)` are `pow(double, double)` (`r_powf`).
fn vo_gradients(pts: &[Vec3], tets: &[[usize; 4]], free: usize) -> Option<(Vec3, Mat3)> {
    let k = vo_stabilisation(pts, tets);
    let p = pts[free];
    let c23 = 2.0 / 3.0 * 0.5f64.r_powf(2.0 / 3.0);
    let c1 = c23 / 3.0;
    let mut grad = Vec3::ZERO;
    let mut hess: Mat3 = [[0.0; 3]; 3];
    // tensor helpers following OpenFOAM's component-wise operators
    let tdiv = |m: Mat3, s: f64| -> Mat3 { m.map(|r| r.map(|x| x / s)) };
    let tmul = |s: f64, m: Mat3| -> Mat3 { m.map(|r| r.map(|x| s * x)) };
    let tsub = |a: Mat3, b: Mat3| -> Mat3 {
        let mut r = a;
        for i in 0..3 {
            for j in 0..3 {
                r[i][j] = a[i][j] - b[i][j];
            }
        }
        r
    };
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
        // Vtri*gradV/stab
        let grad_stab = grad_v.scale(v).div(stab);
        let grad_lsq = p.scale(3.0).sub(a).sub(b).sub(c).scale(2.0);
        let grad_vs = grad_v.add(grad_stab).scale(0.5);
        let vs13 = (2.0 * vs).r_powf(1.0 / 3.0);
        let vstab = vs.r_powf(2.0 / 3.0);
        let sqr_vstab = vstab * vstab;
        // C*(2.*gradVs)/Vs13
        let grad_vstab = grad_vs.scale(2.0).scale(c23).div(vs13);
        // gradF += gradLsq/Vstab - LSqrTri*gradVstab/sqrVstab
        grad = grad.add(grad_lsq.div(vstab).sub(grad_vstab.scale(lsq).div(sqr_vstab)));
        // (gradV*gradV)/stab - sqr(Vtri)*(gradV*gradV)/pow(stab, 3)
        let gvgv = outer(grad_v, grad_v);
        let grad_grad_stab = tsub(tdiv(gvgv, stab), tdiv(tmul(v * v, gvgv), stab.r_powf(3.0)));
        // C*(gradGradStab/Vs13) - C1*4.*(gradVs*gradVs)/pow(Vs13, 4)
        let grad_grad_vstab = tsub(tmul(c23, tdiv(grad_grad_stab, vs13)), tdiv(tmul(c1 * 4.0, outer(grad_vs, grad_vs)), vs13.r_powf(4.0)));
        // gradGradLsq/Vstab (gradGradLsq = 6 I)
        let mut t1: Mat3 = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                t1[i][j] = (if i == j { 6.0 } else { 0.0 }) / vstab;
            }
        }
        // twoSymm(gradLsq*(gradVstab/sqrVstab))
        let gl_gv = outer(grad_lsq, grad_vstab.div(sqr_vstab));
        let mut t2: Mat3 = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                t2[i][j] = if i == j { 2.0 * gl_gv[i][i] } else { gl_gv[i][j] + gl_gv[j][i] };
            }
        }
        // LSqrTri*gradGradVstab/sqrVstab
        let t3 = tdiv(tmul(lsq, grad_grad_vstab), sqr_vstab);
        // 2.0*LSqrTri*(gradVstab*gradVstab)/(sqrVstab*Vstab)
        let t4 = tdiv(tmul(2.0 * lsq, outer(grad_vstab, grad_vstab)), sqr_vstab * vstab);
        let expr = tsub(tsub(t1, t2), t3);
        for i in 0..3 {
            for j in 0..3 {
                hess[i][j] += expr[i][j] + t4[i][j];
            }
        }
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
                    // `triangle(a, b, c).normal()` is the area vector `0.5*((b - a)^(c - a))`;
                    // `disp += 0.01*(n/d)` (gh:#1385 round 4: the port scaled the doubled vector by 0.01/m)
                    let n = b.sub(a).cross(c.sub(a)).scale(0.5);
                    let m = n.length();
                    if m > VSMALL {
                        disp = disp.add(n.div(m).scale(0.01));
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
        *p = p.div(scale);
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
            normals.push(n.div(d));
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

/// The boundary edges of a simplex: edges of the opposite faces (the first
/// three points of each tetrahedron) that only one opposite face uses
/// (upstream's `DynList<edge>` with `numAppearances == 1`; `edge` compares
/// unordered). In first-seen order, as upstream.
fn simplex_boundary_edges(tets: &[[usize; 4]]) -> Vec<(usize, usize)> {
    let mut edges: Vec<((usize, usize), usize)> = Vec::new();
    for t in tets {
        for i in 0..3 {
            let (a, b) = (t[i], t[(i + 1) % 3]);
            match edges.iter_mut().find(|((x, y), _)| (*x == a && *y == b) || (*x == b && *y == a)) {
                Some(e) => e.1 += 1,
                None => edges.push(((a, b), 1)),
            }
        }
    }
    edges.into_iter().filter(|e| e.1 == 1).map(|e| e.0).collect()
}

/// The per-node step of `optimiseBoundaryVolumeOptimizer(nonShrinking =
/// true)`: run the volume optimiser, then keep only the part of the move
/// that stays on the node's surface or edge (`None` at a corner).
fn boundary_volume_non_shrinking(pts: &mut [Vec3], tets: &[[usize; 4]], c: usize) -> Option<Vec3> {
    use outram_foam_basic_lib::primitives::eigen::{eigen_values_symm, eigen_vectors_symm_with};
    use outram_foam_basic_lib::primitives::SymmTensor;
    let orig = pts[c];
    volume_optimizer(pts, tets, c, 1e-5);
    // symmTensor nt = Σ symm(n n) over the boundary triangles
    // (edge.start, edge.end, points[nodeI]); OpenFOAM's triangle::normal is
    // the area vector, normalised here with + VSMALL as upstream.
    let (mut xx, mut xy, mut xz, mut yy, mut yz, mut zz) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for (a, b) in simplex_boundary_edges(tets) {
        let (pa, pb) = (pts[a], pts[b]);
        let n = pb.sub(pa).cross(orig.sub(pa)).scale(0.5);
        let n = n.div(n.length() + VSMALL);
        xx += n.x * n.x;
        xy += n.x * n.y;
        xz += n.x * n.z;
        yy += n.y * n.y;
        yz += n.y * n.z;
        zz += n.z * n.z;
    }
    let nt = SymmTensor::new(xx, xy, xz, yy, yz, zz);
    let ev = eigen_values_symm(nt);
    let mut disp = pts[c].sub(orig);
    let unit = |v: Vec3| v.div(v.length() + VSMALL);
    if ev.z.abs() > ev.y.abs() + ev.x.abs() {
        // ordinary surface vertex
        let e = eigen_vectors_symm_with(nt, ev).row_z();
        let normal = unit(Vec3::new(e.x, e.y, e.z));
        disp = disp.sub(normal.scale(disp.dot(normal)));
    } else if ev.y.abs() > 0.5 * (ev.z.abs() + ev.x.abs()) {
        // a vertex on an edge
        let vecs = eigen_vectors_symm_with(nt, ev);
        let (e1, e2) = (vecs.row_y(), vecs.row_z());
        let n1 = unit(Vec3::new(e1.x, e1.y, e1.z));
        let n2 = unit(Vec3::new(e2.x, e2.y, e2.z));
        let e_vec = unit(n1.cross(n2));
        disp = e_vec.scale(disp.dot(e_vec));
    } else {
        // a corner: do not move it
        return None;
    }
    Some(orig.add(disp))
}

/// The per-node step of `optimiseBoundarySurfaceLaplace`: the mean of the
/// centres of the triangles (boundary edge, node); `None` when the simplex
/// has no boundary edge.
fn boundary_surface_laplace(pts: &mut [Vec3], tets: &[[usize; 4]], c: usize) -> Option<Vec3> {
    let mut new_p = Vec3::ZERO;
    let mut counter = 0usize;
    for (a, b) in simplex_boundary_edges(tets) {
        new_p = new_p.add(pts[a].add(pts[b]).add(pts[c]).scale(1.0 / 3.0));
        counter += 1;
    }
    (counter != 0).then(|| new_p.div(counter as f64))
}

/// cfMesh's checks on the faces flagged `active`; returns the faces failing.
/// `low_quality` adds `checkFaceDotProduct(65°)` and
/// `checkFaceSkewness(2.0)` (`findLowQualityFaces`); otherwise it is
/// `findBadFaces` (pyramids, flatness 0.8, cell-part tetrahedra, areas) or,
/// with `relaxed`, `findBadFacesRelaxed` (pyramids and areas only).
fn flag_faces(mesh: &VolumeMesh, active: &[bool], mode: CheckMode) -> Vec<bool> {
    use crate::poly_mesh_gen_checks as c;
    // gh:#1385 round 3: cfMesh's own checks on the polyMeshGen addressing's
    // face centres, face areas and cell centres (`pmg_geometry`), not this
    // module's vertex-average approximations
    let g = crate::checks::pmg_geometry(mesh);
    let a = Some(active);
    let mut bad = vec![false; mesh.face_count()];
    let mut or = |flags: Vec<bool>| {
        for (b, f) in bad.iter_mut().zip(flags) {
            *b |= f;
        }
    };
    match mode {
        CheckMode::Bad => {
            or(c::check_face_pyramids_g(mesh, &g, VSMALL, a).1);
            or(c::check_face_flatness_g(mesh, &g, 0.8, a).1);
            or(c::check_cell_part_tetrahedra_g(mesh, &g, VSMALL, a).1);
            or(c::check_face_areas_g(mesh, &g, VSMALL, a).1);
        }
        CheckMode::Relaxed => {
            or(c::check_face_pyramids_g(mesh, &g, VSMALL, a).1);
            or(c::check_face_areas_g(mesh, &g, VSMALL, a).1);
        }
        CheckMode::LowQuality => {
            or(c::check_face_dot_product_g(mesh, &g, 65.0, a).1);
            or(c::check_face_skewness_g(mesh, &g, 2.0, a).1);
        }
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
/// The bad faces of `mesh` ([`find_bad_faces`]) and how many of them have
/// a boundary point (which this port's optimisers never move).
pub fn bad_faces_at_the_boundary(mesh: &VolumeMesh) -> (usize, usize) {
    let bad = find_bad_faces(mesh);
    let mut on_bnd = vec![false; mesh.points.len()];
    for f in 0..mesh.face_count() {
        if mesh.neighbour[f].is_none() {
            for &p in &mesh.faces[f] {
                on_bnd[p] = true;
            }
        }
    }
    let n = bad.iter().filter(|&&b| b).count();
    let touch = (0..mesh.face_count()).filter(|&f| bad[f] && mesh.faces[f].iter().any(|&p| on_bnd[p])).count();
    (n, touch)
}

pub fn find_low_quality_faces(mesh: &VolumeMesh) -> Vec<bool> {
    flag_faces(mesh, &vec![true; mesh.face_count()], CheckMode::LowQuality)
}

/// What one optimiser call did.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OptimiserReport {
    /// Faces flagged at the first check.
    pub flagged_before: usize,
    /// Faces flagged at the last check.
    pub flagged_after: usize,
    /// Inner iterations run.
    pub iterations: usize,
    /// Global iterations run (`untangleMeshFV` only).
    pub global_iterations: usize,
    /// Passes of `untangleMeshFV`'s boundary loop that moved boundary points
    /// ([`UntangleControls::boundary_moves`], gh:#1255).
    pub surface_iterations: usize,
    /// Movable boundary nodes summed over those passes' regions.
    pub boundary_nodes_smoothed: usize,
    /// Smoothable nodes summed over every region built (gh:#1241 profile).
    pub nodes_smoothed: usize,
    /// Seconds in the checks, the region builds and the smoothers
    /// (native only; NaN on wasm, which has no clock).
    pub seconds_check: f64,
    /// See [`Self::seconds_check`].
    pub seconds_build: f64,
    /// See [`Self::seconds_check`].
    pub seconds_smooth: f64,
}

/// A clock for the profile (gh:#1241): `std::time::Instant` natively;
/// wasm32-unknown-unknown has none, so the times there are NaN.
#[cfg(not(target_arch = "wasm32"))]
fn stamp() -> Option<std::time::Instant> {
    Some(std::time::Instant::now())
}
#[cfg(target_arch = "wasm32")]
fn stamp() -> Option<()> {
    None
}
#[cfg(not(target_arch = "wasm32"))]
fn since(t: Option<std::time::Instant>) -> f64 {
    t.map_or(f64::NAN, |t| t.elapsed().as_secs_f64())
}
#[cfg(target_arch = "wasm32")]
fn since(_t: Option<()>) -> f64 {
    f64::NAN
}

/// With `GH1385_TRACE` set, print the flagged faces as upstream's instrumented
/// `optimizeMeshFV.C` does (`BADF <tag> <iteration> <n>: <labels>`, gh:#1385).
fn trace_bad(tag: &str, iter: usize, bad: &[bool]) {
    if std::env::var("GH1385_TRACE").is_err() {
        return;
    }
    let labels: Vec<String> = bad.iter().enumerate().filter(|(_, &b)| b).map(|(f, _)| f.to_string()).collect();
    eprintln!("BADF {tag} {iter} {}: {}", labels.len(), labels.join(" "));
}

/// With `GH1385_TRACE` set, print a part tet mesh's nodes as upstream's
/// instrumented `optimizeMeshFV.C` does (`TN <tag> <node> <x> <y> <z>`).
fn trace_tet(tag: &str, tm: &PartTetMesh) {
    if std::env::var("GH1385_TRACE").is_err() {
        return;
    }
    eprintln!("TETMESH {tag} nodes {} tets {}", tm.points.len(), tm.tets.len());
    for (i, p) in tm.points.iter().enumerate() {
        eprintln!("TN {tag} {i} {:.17e} {:.17e} {:.17e}", p.x, p.y, p.z);
    }
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
        let t = stamp();
        let low = flag_faces(mesh, &changed, CheckMode::LowQuality);
        trace_bad("lowq", it, &low);
        rep.seconds_check += since(t);
        let n_bad = low.iter().filter(|&&b| b).count();
        if it == 0 {
            rep.flagged_before = n_bad;
        }
        rep.flagged_after = n_bad;
        rep.iterations = it + 1;
        if n_bad == 0 {
            break;
        }
        let t = stamp();
        let mut tm = PartTetMesh::around_faces(mesh, locked, &low, 2);
        rep.seconds_build += since(t);
        rep.nodes_smoothed += tm.n_smooth_nodes();
        let t = stamp();
        trace_tet("lq_in", &tm);
        tm.optimise_using_volume_optimizer(10);
        trace_tet("lq_vopt", &tm);
        rep.seconds_smooth += since(t);
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
    max_surface_iterations: usize,
    relaxed: bool,
) -> OptimiserReport {
    untangle_mesh_fv_bounded(mesh, locked, max_global_iterations, max_iterations, max_surface_iterations, relaxed, false)
}

/// [`untangle_mesh_fv`] with an optional **stop on a stalled global
/// iteration** (gh:#1241): with `stop_on_stall`, the global loop ends when
/// a global iteration leaves as many bad faces as the one before (or as it
/// started with). Upstream moves boundary points between its global
/// iterations (`optimiseBoundaryVolumeOptimizer`,
/// `optimiseBoundarySurfaceLaplace`), which is what can change the outcome
/// of the next one; this function does not (see [`untangle_mesh_fv_with`]
/// and [`UntangleControls::boundary_moves`] for the port that does), so
/// after a stalled global iteration the next only repeats the interior
/// smoothing on a larger region. Measured on the demo pipe (gh:#1241
/// record): a final untangle went 29 → 29 bad faces over all 10 global
/// iterations, 51 s. The quality targets (`findBadFaces`) are unchanged;
/// only the number of futile passes is.
pub fn untangle_mesh_fv_bounded(
    mesh: &mut VolumeMesh,
    locked: &[bool],
    max_global_iterations: usize,
    max_iterations: usize,
    max_surface_iterations: usize,
    relaxed: bool,
    stop_on_stall: bool,
) -> OptimiserReport {
    let c = UntangleControls { max_global_iterations, max_iterations, max_surface_iterations, relaxed, stop_on_stall, ..UntangleControls::default() };
    untangle_mesh_fv_with(mesh, locked, &c)
}

/// The arguments of `meshOptimizer::untangleMeshFV`, and this port's
/// switches beside them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UntangleControls {
    /// `maxNumGlobalIterations` (upstream default 10).
    pub max_global_iterations: usize,
    /// `maxNumIterations` (upstream default 50).
    pub max_iterations: usize,
    /// `maxNumSurfaceIterations` (upstream default 2): passes of the
    /// boundary loop per global iteration. Used only with
    /// [`Self::boundary_moves`].
    pub max_surface_iterations: usize,
    /// `relaxedCheck`: `findBadFacesRelaxed` instead of `findBadFaces`.
    pub relaxed: bool,
    /// End the global loop on a global iteration that removes no bad face
    /// (gh:#1241; not upstream). Checked after the boundary loop.
    pub stop_on_stall: bool,
    /// Run upstream's boundary loop between global iterations (gh:#1255):
    /// `optimiseBoundaryVolumeOptimizer(true)` while `nGlobalIter < 2`,
    /// `optimiseBoundarySurfaceLaplace` while `< 5`, then
    /// `optimiseBoundaryVolumeOptimizer(false)`, on the cells around the bad
    /// faces (no extra layer), up to `max_surface_iterations` passes. `false`
    /// is this port's behaviour before gh:#1255 (boundary points never
    /// move) and keeps every recorded result; upstream always runs it.
    pub boundary_moves: bool,
    /// Run the LP `meshUntangler` between the Knupp untangler and the volume
    /// optimiser (`tmo.optimiseUsingMeshUntangler()`, gh:#1260). `false`
    /// is this port's behaviour before gh:#1260; upstream always runs it.
    pub mesh_untangler: bool,
}

impl Default for UntangleControls {
    /// Upstream's defaults, with this port's switches off.
    fn default() -> Self {
        UntangleControls { max_global_iterations: 10, max_iterations: 50, max_surface_iterations: 2, relaxed: false, stop_on_stall: false, boundary_moves: false, mesh_untangler: false }
    }
}

/// `meshOptimizer::untangleMeshFV`, with [`UntangleControls`].
///
/// Per global iteration: flag the bad faces among those changed (all, at
/// first), build the region around them grown by `nGlobalIter / 2 + 1`
/// layers, and run the Knupp untangler then the volume optimiser; stop when
/// no bad face is left, or five passes after the fewest bad faces, or after
/// `max_iterations`. Then, with [`UntangleControls::boundary_moves`], the
/// boundary loop (`while (nIter++ < maxNumSurfaceIterations)`): check, and
/// move the boundary nodes of the region around the bad faces. Upstream's
/// `enforceConstraints_` branch (stop when a boundary move would be needed)
/// belongs to the lock API (gh:#1256). The LP mesh untangler is not ported
/// yet (gh:#1260).
pub fn untangle_mesh_fv_with(mesh: &mut VolumeMesh, locked: &[bool], controls: &UntangleControls) -> OptimiserReport {
    untangle_impl(mesh, locked, controls, false, &mut |_: &mut VolumeMesh, _: &[bool]| {}).0
}

/// [`untangle_mesh_fv_with`] with a **projector** called after every pass
/// of the boundary loop (gh:#1283): `project(mesh, moved)` gets the mesh
/// and the points that pass moved, and may move them again (back onto the
/// surface, [`crate::final_mesh::BoundaryProjection::EachPass`]). The faces
/// of the moved points are already marked changed, so the next global
/// iteration's interior passes see whatever the projection does to them.
/// **Not upstream:** `untangleMeshFV` has no such step (it keeps a point on
/// its surface only through `optimiseBoundaryVolumeOptimizer(true)`'s
/// tangent-plane constraint in global iterations 0 and 1). Also returns
/// the points any boundary pass moved. With a projector that does nothing
/// this is [`untangle_mesh_fv_with`], bit for bit.
pub fn untangle_mesh_fv_projected<P: FnMut(&mut VolumeMesh, &[bool])>(mesh: &mut VolumeMesh, locked: &[bool], controls: &UntangleControls, project: &mut P) -> (OptimiserReport, Vec<bool>) {
    let mut moved = vec![false; mesh.points.len()];
    let mut hook = |m: &mut VolumeMesh, pass: &[bool]| {
        for (a, &b) in moved.iter_mut().zip(pass) {
            *a |= b;
        }
        project(m, pass);
    };
    let rep = untangle_impl(mesh, locked, controls, false, &mut hook).0;
    (rep, moved)
}

/// [`untangle_mesh_fv_with`]; with `enforce_constraints` (and
/// `boundary_moves`), stop at the boundary loop's first check that finds a
/// bad face and return those faces (upstream's `enforceConstraints_`
/// branch).
fn untangle_impl<P: FnMut(&mut VolumeMesh, &[bool])>(mesh: &mut VolumeMesh, locked: &[bool], controls: &UntangleControls, enforce_constraints: bool, project: &mut P) -> (OptimiserReport, Option<Vec<bool>>) {
    let c = *controls;
    let pc = point_cells(mesh);
    let cf = cell_faces(mesh);
    let mut changed = vec![true; mesh.face_count()];
    let mode = if c.relaxed { CheckMode::Relaxed } else { CheckMode::Bad };
    let mut rep = OptimiserReport::default();
    let mut first = true;
    let mut n_global = 0usize;
    let mut bad_before_global = usize::MAX;
    loop {
        let mut n_iter = 0usize;
        let (mut min_bad, mut min_iter) = (usize::MAX, 0usize);
        let mut n_bad;
        loop {
            let t = stamp();
            let bad = flag_faces(mesh, &changed, mode);
            rep.seconds_check += since(t);
            n_bad = bad.iter().filter(|&&b| b).count();
            trace_bad("untangle", n_iter, &bad);
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
            let t = stamp();
            let mut tm = PartTetMesh::around_faces(mesh, locked, &bad, n_global / 2 + 1);
            rep.seconds_build += since(t);
            rep.nodes_smoothed += tm.n_smooth_nodes();
            let t = stamp();
            trace_tet("in", &tm);
            tm.optimise_using_knupp_metric(5);
            trace_tet("knupp", &tm);
            if c.mesh_untangler {
                tm.optimise_using_mesh_untangler(5);
            }
            trace_tet("untangler", &tm);
            tm.optimise_using_volume_optimizer(10);
            trace_tet("vopt", &tm);
            rep.seconds_smooth += since(t);
            changed = tm.update_orig(mesh, &pc, &cf);
            rep.iterations += 1;
            n_iter += 1;
            if !(n_iter < min_iter + 5 + 1 && n_iter < c.max_iterations) {
                break;
            }
        }
        n_global += 1;
        rep.global_iterations = n_global;
        if n_bad == 0 || n_global >= c.max_global_iterations {
            break;
        }
        if c.boundary_moves {
            // move boundary vertices
            let mut s_iter = 0usize;
            while s_iter < c.max_surface_iterations {
                s_iter += 1;
                let t = stamp();
                let bad = flag_faces(mesh, &changed, mode);
                rep.seconds_check += since(t);
                n_bad = bad.iter().filter(|&&b| b).count();
                rep.flagged_after = n_bad;
                if n_bad == 0 {
                    break;
                }
                if enforce_constraints {
                    return (rep, Some(bad));
                }
                let t = stamp();
                let mut tm = PartTetMesh::around_faces(mesh, locked, &bad, 0);
                rep.seconds_build += since(t);
                rep.boundary_nodes_smoothed += tm.n_boundary_nodes();
                let t = stamp();
                if n_global < 2 {
                    // the point stays in the plane determined by the point normal
                    tm.optimise_boundary_volume_optimizer(3, true);
                } else if n_global < 5 {
                    // move points without any constraints on the movement
                    tm.optimise_boundary_surface_laplace(3);
                } else {
                    // move boundary points without any constraints
                    tm.optimise_boundary_volume_optimizer(3, false);
                }
                rep.seconds_smooth += since(t);
                let before = mesh.points.clone();
                changed = tm.update_orig(mesh, &pc, &cf);
                let pass: Vec<bool> = before.iter().zip(&mesh.points).map(|(a, b)| a != b).collect();
                project(mesh, &pass);
                rep.surface_iterations += 1;
            }
        }
        let start = if bad_before_global == usize::MAX { rep.flagged_before } else { bad_before_global };
        if c.stop_on_stall && n_bad >= start {
            break;
        }
        bad_before_global = n_bad;
    }
    (rep, None)
}

/// `meshOptimizer::optimizeMeshNearBoundaries(maxNumIterations,
/// numLayersOfCells)`: one region, the boundary cells grown
/// `num_layers_of_cells` times ([`PartTetMesh::near_boundary`]), and
/// `max_iterations` single passes of the volume optimiser on it, writing
/// back after each (`do … while (++nIter < maxNumIterations)`: at least
/// one). Boundary points do not move (they are `BOUNDARY`, not `SMOOTH`).
/// Upstream defaults: 2 iterations, 2 layers.
pub fn optimize_mesh_near_boundaries(mesh: &mut VolumeMesh, locked: &[bool], max_iterations: usize, num_layers_of_cells: usize) -> OptimiserReport {
    let pc = point_cells(mesh);
    let cf = cell_faces(mesh);
    let mut rep = OptimiserReport::default();
    let t = stamp();
    let mut tm = PartTetMesh::near_boundary(mesh, locked, num_layers_of_cells);
    rep.seconds_build += since(t);
    rep.nodes_smoothed = tm.n_smooth_nodes();
    loop {
        let t = stamp();
        tm.optimise_using_volume_optimizer(1);
        rep.seconds_smooth += since(t);
        let _ = tm.update_orig(mesh, &pc, &cf);
        rep.iterations += 1;
        if rep.iterations >= max_iterations {
            break;
        }
    }
    rep
}

/// cfMesh `meshOptimizer::optimizeSurface(octree)` (gh:#1259):
/// `meshSurfaceOptimizer::optimizeSurface()` on the boundary of `mesh`
/// (boundary face `f` in surface region `face_region[f]`), then every
/// boundary point mapped back onto its patches
/// (`mapVerticesOntoSurfacePatches`). Opt-in: no default path calls it.
pub fn optimize_surface(
    mesh: &mut VolumeMesh,
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: &crate::patches::SurfaceRegions,
    face_region: &[Option<usize>],
) -> crate::surface_optimizer::UntangleReport {
    let rep = crate::surface_optimizer::optimise_mesh_surface(mesh, points, tris, regions, face_region);
    *mesh = crate::surface_mapper::map_vertices_onto_surface_patches_given(mesh, points, tris, regions, face_region);
    rep
}

/// [`optimize_surface`] with the searches through the octree, as upstream's
/// `meshOptimizer::optimizeSurface(octree)` (gh:#1385): the surface optimiser
/// and the mapper both take it.
pub fn optimize_surface_octree(
    mesh: &mut VolumeMesh,
    o: &crate::mesh_octree::MeshOctree,
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: &crate::patches::SurfaceRegions,
    face_region: &[Option<usize>],
) -> crate::surface_optimizer::UntangleReport {
    let rep = crate::surface_optimizer::optimise_mesh_surface_octree(mesh, o, points, tris, regions, face_region);
    *mesh = crate::surface_mapper::map_vertices_onto_surface_patches_given_octree(mesh, o, points, tris, regions, face_region);
    rep
}

/// `polyMeshGenChecks::checkFaceDotProduct(mesh, values, changedFacePtr)`:
/// `d · S / (|d| |S| + VSMALL)` on the active internal faces, 1 elsewhere.
pub fn face_dot_products(mesh: &VolumeMesh, active: &[bool]) -> Vec<f64> {
    face_dot_products_g(mesh, &crate::checks::pmg_geometry(mesh), active)
}

/// [`face_dot_products`] on a precomputed [`crate::checks::PmgGeometry`]:
/// `(d & s)/(mag(d)*mag(s) + VSMALL)` with the addressing's cell centres and
/// face areas.
pub(crate) fn face_dot_products_g(mesh: &VolumeMesh, g: &crate::checks::PmgGeometry, active: &[bool]) -> Vec<f64> {
    (0..mesh.face_count())
        .map(|f| match mesh.neighbour[f] {
            Some(n) if active[f] => {
                let d = g.cell_centres[n].sub(g.cell_centres[mesh.owner[f]]);
                let s = g.face_areas[f];
                d.dot(s) / (d.length() * s.length() + VSMALL)
            }
            _ => 1.0,
        })
        .collect()
}

/// `polyMeshGenChecks::checkFaceSkewness(mesh, values, changedFacePtr)`,
/// cfMesh's definition: on the active internal faces, the distance of the
/// face centre from the distance-weighted point of the owner–neighbour line
/// over the centre distance; on **every** boundary face (upstream does not
/// test `changedFacePtr` there), the tangential part of centre-to-face over
/// its length; 0 elsewhere.
pub fn face_skewness(mesh: &VolumeMesh, active: &[bool]) -> Vec<f64> {
    face_skewness_g(mesh, &crate::checks::pmg_geometry(mesh), active)
}

/// [`face_skewness`] on a precomputed [`crate::checks::PmgGeometry`], in
/// upstream's operation order: `centres[own]*dNei/(dOwn + dNei) +
/// centres[nei]*dOwn/(dOwn + dNei)` is `((c*d)/s) + ((c*d)/s)`; the boundary
/// normal is `face::normal` (`areaNormal`), divided by its magnitude.
pub(crate) fn face_skewness_g(mesh: &VolumeMesh, g: &crate::checks::PmgGeometry, active: &[bool]) -> Vec<f64> {
    (0..mesh.face_count())
        .map(|f| {
            let fc = g.face_centres[f];
            let co = g.cell_centres[mesh.owner[f]];
            match mesh.neighbour[f] {
                Some(n) => {
                    if !active[f] {
                        return 0.0;
                    }
                    let cn = g.cell_centres[n];
                    let d_own = fc.sub(co).length();
                    let d_nei = fc.sub(cn).length();
                    let x = co.scale(d_nei).div(d_own + d_nei).add(cn.scale(d_own).div(d_own + d_nei));
                    fc.sub(x).length() / (cn.sub(co).length() + VSMALL)
                }
                None => {
                    let d = fc.sub(co);
                    let s = crate::surface_engine::ring_area_vector(&mesh.points, &mesh.faces[f]);
                    let m = s.length();
                    if m <= VSMALL {
                        return 0.0;
                    }
                    let nrm = s.div(m);
                    let dn = nrm.scale(nrm.dot(d));
                    d.sub(dn).length() / (d.length() + VSMALL)
                }
            }
        })
        .collect()
}

/// `polyMeshGenChecks::findWorstQualityFaces(mesh, badFaces, report,
/// activeFacePtr, relativeThreshold)`: among the active faces, those whose
/// dot product is below `min + t (1 − min)` or whose skewness is above
/// `(1 − t) max`, the minimum and maximum taken over every face's value
/// (inactive faces count as 1 and 0, as upstream's initialised fields).
pub fn find_worst_quality_faces(mesh: &VolumeMesh, active: &[bool], relative_threshold: f64) -> Vec<bool> {
    let dots = face_dot_products(mesh, active);
    let min_dot = dots.iter().copied().fold(f64::MAX, f64::min);
    let warn_dot = min_dot + relative_threshold * (1.0 - min_dot);
    let mut bad: Vec<bool> = (0..mesh.face_count()).map(|f| active[f] && dots[f] < warn_dot).collect();
    let skew = face_skewness(mesh, active);
    let max_skew = skew.iter().copied().fold(f64::MIN, f64::max);
    let warn_skew = (1.0 - relative_threshold) * max_skew;
    for f in 0..mesh.face_count() {
        if active[f] && skew[f] > warn_skew {
            bad[f] = true;
        }
    }
    bad
}

/// `meshOptimizer::optimizeMeshFVBestQuality(maxNumIterations, threshold)`:
/// repeatedly select the worst faces ([`find_worst_quality_faces`]) among
/// those changed and run 20 volume-optimiser passes on the cells two point
/// layers around them, until none is selected, five passes after the
/// fewest, or `max_iterations`. Upstream defaults: 50 iterations,
/// threshold 0.1.
pub fn optimize_mesh_fv_best_quality(mesh: &mut VolumeMesh, locked: &[bool], max_iterations: usize, threshold: f64) -> OptimiserReport {
    let pc = point_cells(mesh);
    let cf = cell_faces(mesh);
    let mut changed = vec![true; mesh.face_count()];
    let mut rep = OptimiserReport::default();
    let (mut min_bad, mut min_iter) = (usize::MAX, 0usize);
    let mut n_iter = 0usize;
    loop {
        let t = stamp();
        let worst = find_worst_quality_faces(mesh, &changed, threshold);
        rep.seconds_check += since(t);
        let n_bad = worst.iter().filter(|&&b| b).count();
        if n_iter == 0 {
            rep.flagged_before = n_bad;
        }
        rep.flagged_after = n_bad;
        if n_bad == 0 {
            break;
        }
        if n_bad < min_bad {
            min_bad = n_bad;
            min_iter = n_iter;
        }
        let t = stamp();
        let mut tm = PartTetMesh::around_faces(mesh, locked, &worst, 2);
        rep.seconds_build += since(t);
        rep.nodes_smoothed += tm.n_smooth_nodes();
        let t = stamp();
        tm.optimise_using_volume_optimizer(20);
        rep.seconds_smooth += since(t);
        changed = tm.update_orig(mesh, &pc, &cf);
        rep.iterations += 1;
        // while ((nIter < minIter + 5) && (++nIter < maxNumIterations))
        if n_iter >= min_iter + 5 {
            break;
        }
        n_iter += 1;
        if n_iter >= max_iterations {
            break;
        }
    }
    rep
}

/// `meshOptimizer::vertexType_` (the `vertexLocation_` flags).
pub mod vertex_type {
    /// Not on the boundary.
    pub const INSIDE: u8 = 1;
    /// On the boundary.
    pub const BOUNDARY: u8 = 2;
    /// On a feature edge (not set here: see [`super::MeshOptimizer::new`]).
    pub const EDGE: u8 = 4;
    /// A feature corner (not set here: see [`super::MeshOptimizer::new`]).
    pub const CORNER: u8 = 8;
    /// Locked by the user (`lockCells`, `lockFaces`, `lockPoints`).
    pub const LOCKED: u8 = 32;
}

/// Which of `meshOptimizer::laplaceSmoother`'s smoothers to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaplaceKind {
    /// `laplacian`: the mean of the point's edge neighbours
    /// (`pointPoints`), updated in place (Gauss–Seidel), as upstream.
    Laplacian,
    /// `laplacianPC`: the mean of the centres of the point's cells.
    PointCells,
    /// `laplacianWPC`: the volume-weighted mean of those centres
    /// (weights `max(V, VSMALL)`).
    WeightedPointCells,
}

/// `pointPoints`: each point's edge neighbours, ascending (upstream builds
/// them from `edges`; the order only changes the round-off of the sum).
fn point_points(mesh: &VolumeMesh) -> Vec<Vec<usize>> {
    let mut pp: Vec<Vec<usize>> = vec![Vec::new(); mesh.points.len()];
    for ring in &mesh.faces {
        let k = ring.len();
        for i in 0..k {
            let (a, b) = (ring[i], ring[(i + 1) % k]);
            pp[a].push(b);
            pp[b].push(a);
        }
    }
    for v in pp.iter_mut() {
        v.sort_unstable();
        v.dedup();
    }
    pp
}

/// `meshOptimizer::laplaceSmoother::{laplacian, laplacianPC, laplacianWPC}`
/// on `smooth_points` (upstream: every `INSIDE` point), skipping `LOCKED`
/// ones, `n_iterations` times.
///
/// - `Laplacian` moves each point to the mean of its edge neighbours in
///   place, so a later point sees an earlier one's new position. Upstream
///   `return`s from the whole smoother at a point with no neighbour (no
///   geometry update); so does this.
/// - `PointCells` / `WeightedPointCells` read the cell centres (and
///   volumes) of the start of each iteration (`updateMeshGeometry` runs
///   after it), so the points of one iteration do not see each other.
///
/// ~~Centres are this crate's ([`crate::checks::cell_geometry`]), not
/// OpenFOAM's area-weighted ones (module docs).~~ **CORRECTED 2026-11-11
/// (gh:#1385):** cell centres are cfMesh's (`foam_cell_centres_and_vols`).
/// Not
/// [`crate::smooth::laplacian_smooth`] (Freitag's smart Laplacian): this is
/// cfMesh's plain one, which may invert a cell; `optimizeMeshFV` untangles
/// after it.
pub fn laplace_smoother(mesh: &mut VolumeMesh, vertex_location: &[u8], smooth_points: &[usize], kind: LaplaceKind, n_iterations: usize) {
    let unlocked = |p: usize| vertex_location[p] & vertex_type::LOCKED == 0;
    match kind {
        LaplaceKind::Laplacian => {
            let pp = point_points(mesh);
            for _ in 0..n_iterations {
                for &p in smooth_points {
                    if !unlocked(p) {
                        continue;
                    }
                    if pp[p].is_empty() {
                        return;
                    }
                    let mut new_p = Vec3::ZERO;
                    for &q in &pp[p] {
                        new_p = new_p.add(mesh.points[q]);
                    }
                    mesh.points[p] = new_p.div(pp[p].len() as f64);
                }
            }
        }
        LaplaceKind::PointCells | LaplaceKind::WeightedPointCells => {
            let pc = point_cells(mesh);
            for _ in 0..n_iterations {
                let (vols, centres) = crate::checks::foam_cell_centres_and_vols(mesh);
                for &p in smooth_points {
                    if !unlocked(p) || pc[p].is_empty() {
                        continue;
                    }
                    let mut new_p = Vec3::ZERO;
                    if kind == LaplaceKind::PointCells {
                        for &c in &pc[p] {
                            new_p = new_p.add(centres[c]);
                        }
                        new_p = new_p.div(pc[p].len() as f64);
                    } else {
                        let mut sum_w = 0.0;
                        for &c in &pc[p] {
                            let w = vols[c].max(VSMALL);
                            new_p = new_p.add(centres[c].scale(w));
                            sum_w += w;
                        }
                        new_p = new_p.div(sum_w);
                    }
                    mesh.points[p] = new_p;
                }
            }
        }
    }
}

/// `meshOptimizer::laplaceSmoother::laplacianSurface`: each point to the
/// mean of its edge neighbours that are not `INSIDE` (a boundary point
/// stays among boundary points), in place. No upstream workflow calls it
/// (`grep` of `meshLibrary/`: only the class), ported for completeness.
pub fn laplace_smoother_surface(mesh: &mut VolumeMesh, vertex_location: &[u8], smooth_points: &[usize], n_iterations: usize) {
    let pp = point_points(mesh);
    for _ in 0..n_iterations {
        for &p in smooth_points {
            if vertex_location[p] & vertex_type::LOCKED != 0 {
                continue;
            }
            let mut new_p = Vec3::ZERO;
            let mut counter = 0usize;
            for &q in &pp[p] {
                if vertex_location[q] & vertex_type::INSIDE != 0 {
                    continue;
                }
                new_p = new_p.add(mesh.points[q]);
                counter += 1;
            }
            if counter != 0 {
                mesh.points[p] = new_p.scale(1.0 / counter as f64);
            }
        }
    }
}

/// `meshOptimizer`'s state: the point classes and the user's locks
/// (`vertexLocation_`, `lockedFaces_`, `enforceConstraints_`), with its
/// drivers as methods on a mesh passed in (this crate's rule: no borrowed
/// fields).
///
/// The driver free functions ([`untangle_mesh_fv_with`],
/// [`optimize_low_quality_faces`], …) take the locks as a `locked` slice;
/// these methods pass [`Self::locked_points`].
#[derive(Debug, Clone, PartialEq)]
pub struct MeshOptimizer {
    /// `vertexLocation_`: [`vertex_type`] flags per point.
    pub vertex_location: Vec<u8>,
    /// `lockedFaces_`, in the order they were locked.
    pub locked_faces: Vec<usize>,
    /// `enforceConstraints_`: [`Self::untangle_mesh_fv`] stops instead of
    /// moving boundary points, and reports the points of the faces left.
    pub enforce_constraints: bool,
    /// `badPointsSubsetName_`.
    pub bad_points_subset_name: String,
}

impl MeshOptimizer {
    /// `meshOptimizer(mesh)` / `calculatePointLocations`: every point
    /// `INSIDE`, the boundary points `BOUNDARY`. **Not set:** `EDGE` and
    /// `CORNER` (they need `meshSurfacePartitioner`; only
    /// `meshOptimizer::optimizeSurface` reads them, which is not in this
    /// module) and `PARALLELBOUNDARY` (serial).
    pub fn new(mesh: &VolumeMesh) -> Self {
        let mut vertex_location = vec![vertex_type::INSIDE; mesh.points.len()];
        for f in mesh.n_internal_faces()..mesh.face_count() {
            for &p in &mesh.faces[f] {
                vertex_location[p] = vertex_type::BOUNDARY;
            }
        }
        MeshOptimizer { vertex_location, locked_faces: Vec::new(), enforce_constraints: false, bad_points_subset_name: String::new() }
    }

    /// `enforceConstraints(subsetName)` (upstream default name
    /// `"badPoints"`).
    pub fn set_enforce_constraints(&mut self, subset_name: &str) {
        self.enforce_constraints = true;
        self.bad_points_subset_name = subset_name.to_string();
    }

    /// `lockCells`: lock every face of the cells, and their points.
    pub fn lock_cells(&mut self, mesh: &VolumeMesh, cells: &[usize]) {
        let cf = cell_faces(mesh);
        let mut locked_face = vec![false; mesh.face_count()];
        for &c in cells {
            for &f in &cf[c] {
                locked_face[f] = true;
            }
        }
        self.lock_face_flags(mesh, &locked_face);
    }

    /// `lockFaces`: lock the faces and their points.
    pub fn lock_faces(&mut self, mesh: &VolumeMesh, faces: &[usize]) {
        let mut locked_face = vec![false; mesh.face_count()];
        for &f in faces {
            locked_face[f] = true;
        }
        self.lock_face_flags(mesh, &locked_face);
    }

    /// The common tail of `lockCells` and `lockFaces`.
    fn lock_face_flags(&mut self, mesh: &VolumeMesh, locked_face: &[bool]) {
        for (f, &l) in locked_face.iter().enumerate() {
            if l {
                self.locked_faces.push(f);
                for &p in &mesh.faces[f] {
                    self.vertex_location[p] |= vertex_type::LOCKED;
                }
            }
        }
    }

    /// `lockPoints`.
    pub fn lock_points(&mut self, points: &[usize]) {
        for &p in points {
            self.vertex_location[p] |= vertex_type::LOCKED;
        }
    }

    /// `removeUserConstraints`: clear the locked faces and every `LOCKED`
    /// flag.
    pub fn remove_user_constraints(&mut self) {
        self.locked_faces.clear();
        for v in self.vertex_location.iter_mut() {
            *v &= !vertex_type::LOCKED;
        }
    }

    /// The `LOCKED` points as the `locked` slice the drivers take.
    pub fn locked_points(&self) -> Vec<bool> {
        self.vertex_location.iter().map(|&v| v & vertex_type::LOCKED != 0).collect()
    }

    /// The `INSIDE` points (`optimizeLaplacian*`'s smooth points).
    fn inside_points(&self) -> Vec<usize> {
        (0..self.vertex_location.len()).filter(|&p| self.vertex_location[p] & vertex_type::INSIDE != 0).collect()
    }

    /// `laplaceSmoother(mesh, vertexLocation_).optimizeLaplacian /
    /// optimizeLaplacianPC / optimizeLaplacianWPC(nIterations)` (upstream
    /// default 1): [`laplace_smoother`] on every `INSIDE` point.
    pub fn optimize_laplacian(&self, mesh: &mut VolumeMesh, kind: LaplaceKind, n_iterations: usize) {
        laplace_smoother(mesh, &self.vertex_location, &self.inside_points(), kind, n_iterations);
    }

    /// `meshOptimizer::optimizeMeshFV(numLaplaceIterations,
    /// maxNumGlobalIterations, maxNumIterations, maxNumSurfaceIterations)`
    /// (upstream defaults 5, 10, 50, 2): `laplacianPC` on the inside points,
    /// then [`Self::untangle_mesh_fv`].
    pub fn optimize_mesh_fv(&self, mesh: &mut VolumeMesh, num_laplace_iterations: usize, controls: &UntangleControls) -> Result<OptimiserReport, ConstraintViolation> {
        self.optimize_laplacian(mesh, LaplaceKind::PointCells, num_laplace_iterations);
        crate::pipeline::cartesian_mesh::trace_points("fv_lap", mesh);
        self.untangle_mesh_fv(mesh, controls)
    }

    /// `meshOptimizer::untangleMeshFV` with this optimiser's locks. With
    /// [`Self::enforce_constraints`] and [`UntangleControls::boundary_moves`],
    /// the first boundary pass that finds a bad face stops and returns the
    /// points of the bad faces left (upstream adds them to a point subset
    /// named [`Self::bad_points_subset_name`] and throws).
    pub fn untangle_mesh_fv(&self, mesh: &mut VolumeMesh, controls: &UntangleControls) -> Result<OptimiserReport, ConstraintViolation> {
        let locked = self.locked_points();
        match untangle_impl(mesh, &locked, controls, self.enforce_constraints, &mut |_: &mut VolumeMesh, _: &[bool]| {}) {
            (rep, None) => Ok(rep),
            (rep, Some(bad)) => {
                let mut points: Vec<usize> = (0..mesh.face_count()).filter(|&f| bad[f]).flat_map(|f| mesh.faces[f].clone()).collect();
                points.sort_unstable();
                points.dedup();
                Err(ConstraintViolation { subset_name: self.bad_points_subset_name.clone(), points, report: rep })
            }
        }
    }

    /// `lockCellsInSubset(name)`: [`Self::lock_cells`] on the cells of the
    /// cell subset `name`; `false` (upstream warns "is not a cell subset")
    /// when there is none.
    pub fn lock_cells_in_subset(&mut self, mesh: &VolumeMesh, subsets: &MeshSubsets, name: &str) -> bool {
        match subsets.cells.index(name) {
            Some(id) => {
                self.lock_cells(mesh, &subsets.cells.elements(id));
                true
            }
            None => false,
        }
    }

    /// `lockFacesInSubset(name)`: [`Self::lock_faces`] on the face subset.
    pub fn lock_faces_in_subset(&mut self, mesh: &VolumeMesh, subsets: &MeshSubsets, name: &str) -> bool {
        match subsets.faces.index(name) {
            Some(id) => {
                self.lock_faces(mesh, &subsets.faces.elements(id));
                true
            }
            None => false,
        }
    }

    /// `lockPointsInSubset(name)`: [`Self::lock_points`] on the point
    /// subset. **Upstream defect, not copied:** upstream passes the point
    /// labels to `lockCells` (so it locks the cells whose *indices* equal
    /// those point labels, or reads past the cell list); this locks the
    /// points, which is what the function's name and comment say.
    pub fn lock_points_in_subset(&mut self, subsets: &MeshSubsets, name: &str) -> bool {
        match subsets.points.index(name) {
            Some(id) => {
                self.lock_points(&subsets.points.elements(id));
                true
            }
            None => false,
        }
    }

    /// [`Self::untangle_mesh_fv`] recording what upstream records in the
    /// mesh's subsets: on a [`ConstraintViolation`], the point subset
    /// [`Self::bad_points_subset_name`] with its points; when bad faces are
    /// left at the end, the face subset `badFaces` (replaced if it exists)
    /// and the cell subset `badCells` (added to) with their owners and
    /// neighbours. (Upstream records the set of its last check, over the
    /// changed faces; this records a check of every face.)
    pub fn untangle_mesh_fv_recording(&self, mesh: &mut VolumeMesh, subsets: &mut MeshSubsets, controls: &UntangleControls) -> Result<OptimiserReport, ConstraintViolation> {
        let r = self.untangle_mesh_fv(mesh, controls);
        match &r {
            Err(v) => {
                let id = subsets.points.add(&v.subset_name);
                for &p in &v.points {
                    subsets.points.add_element(id, p);
                }
            }
            Ok(rep) if rep.flagged_after != 0 => {
                let mode = if controls.relaxed { CheckMode::Relaxed } else { CheckMode::Bad };
                let bad = flag_faces(mesh, &vec![true; mesh.face_count()], mode);
                if let Some(old) = subsets.faces.index("badFaces") {
                    subsets.faces.remove(old);
                }
                let fid = subsets.faces.add("badFaces");
                let cid = subsets.cells.add("badCells");
                for f in (0..mesh.face_count()).filter(|&f| bad[f]) {
                    subsets.faces.add_element(fid, f);
                    subsets.cells.add_element(cid, mesh.owner[f]);
                    if let Some(n) = mesh.neighbour[f] {
                        subsets.cells.add_element(cid, n);
                    }
                }
            }
            Ok(_) => {}
        }
        r
    }

    /// `meshOptimizer::optimizeLowQualityFaces` with this optimiser's locks.
    pub fn optimize_low_quality_faces(&self, mesh: &mut VolumeMesh, max_iterations: usize) -> OptimiserReport {
        optimize_low_quality_faces(mesh, &self.locked_points(), max_iterations)
    }
}

/// `untangleMeshFV` under `enforceConstraints`: bad faces were left that
/// only boundary moves could repair (upstream throws
/// `"Cannot untangle mesh!!"` after writing the subset).
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintViolation {
    /// The point subset's name (`badPointsSubsetName_`).
    pub subset_name: String,
    /// The points of the bad faces left, ascending.
    pub points: Vec<usize>,
    /// What the untangler did up to the stop.
    pub report: OptimiserReport,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::cell_geometry;
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

    /// An 8×8×8 unit box (h = 0.125) with the boundary point (0.5, 0.5, 0)
    /// slid along its side to (0.7, 0.6, 0), past its neighbours at
    /// x = 0.625: the side's faces around it fold over, in the plane.
    /// Returns the mesh and the moved point.
    ///
    /// ~~The first fixture pushed the point inward to (0.5, 0.5, 0.2).~~
    /// **CHANGED 2026-11-05 (first run):** the inward dent was repaired by
    /// interior moves alone (4 → 0 bad faces in one pass): its bad faces
    /// were internal, and the dent's own boundary faces never failed
    /// `findBadFaces`. Prediction (A) and the enforce-constraints
    /// prediction missed on it. A fold in the plane gives boundary faces
    /// whose area vectors point into the box, which no interior move can
    /// turn back.
    fn dented_box() -> (VolumeMesh, usize) {
        let mut m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [8, 8, 8]);
        let p = m.points.iter().position(|q| q.sub(Vec3::new(0.5, 0.5, 0.0)).length() < 1e-12).unwrap();
        m.points[p] = Vec3::new(0.7, 0.6, 0.0);
        (m, p)
    }

    fn count(v: &[bool]) -> usize {
        v.iter().filter(|&&b| b).count()
    }

    /// V&V (gh:#1255) — **`untangleMeshFV`'s boundary loop repairs a dent
    /// that interior moves cannot.**
    ///
    /// Methodology: [`dented_box`]. Three runs from the same mesh:
    /// (A) [`untangle_mesh_fv`] (10, 50, 2), interior moves only, as before
    /// gh:#1255; (B) [`untangle_mesh_fv_with`] with
    /// `boundary_moves` and `max_global_iterations` 2, so the only boundary
    /// moves are one global iteration's `optimiseBoundaryVolumeOptimizer
    /// (nonShrinking = true)`; (C) the same with upstream's defaults
    /// (10, 50, 2) and `boundary_moves`.
    ///
    /// Predictions (written 2026-11-05, before the first run):
    /// - (A) leaves ≥ 1 bad face, and every bad face left has a boundary
    ///   point (the dent's own boundary faces are folded, and only a
    ///   boundary move can unfold them).
    /// - (B) every boundary point away from the dent (not the dented point
    ///   and not one of its 8 neighbours on z = 0) stays on its side's plane
    ///   to 1e-12: their boundary faces are flat, so the normal tensor has
    ///   one non-zero eigenvalue (a surface point: the normal component is
    ///   removed) or two (a box edge: only the edge direction is kept), or
    ///   three (a corner, not moved). (Second fixture, written before its
    ///   run: the fold is in the plane z = 0, so every boundary point, the
    ///   folded one included, stays on its plane to 1e-12.)
    /// - (C) leaves 0 bad faces; the 8 box corners are bit-identical (they
    ///   are never in a region: the regions are the cells around the bad
    ///   faces, all near the dent); the run is deterministic.
    ///
    /// Results (2026-11-05): see the `assert`s; the measured counts are
    /// recorded in `docs/upstream_coverage.md` (gh:#1255 section).
    #[test]
    fn boundary_moves_repair_a_dent_interior_moves_cannot() {
        let (m0, dent) = dented_box();
        let locked = vec![false; m0.points.len()];
        let mut on_bnd = vec![false; m0.points.len()];
        for f in m0.n_internal_faces()..m0.face_count() {
            for &p in &m0.faces[f] {
                on_bnd[p] = true;
            }
        }
        let bad0 = count(&find_bad_faces(&m0));
        assert!(bad0 > 0);

        // (A) interior only.
        let mut a = m0.clone();
        let ra = untangle_mesh_fv(&mut a, &locked, 10, 50, 2, false);
        let bad_a = find_bad_faces(&a);
        let left_a = count(&bad_a);
        let at_bnd_a = (0..a.face_count()).filter(|&f| bad_a[f] && a.faces[f].iter().any(|&p| on_bnd[p])).count();
        println!("(A) interior only: {bad0} -> {left_a} bad faces ({at_bnd_a} with a boundary point); {ra:?}");
        assert!(left_a >= 1, "interior moves alone should not repair the dent");
        // Second half of (A) MISSED (2026-11-05, folded fixture): 3 -> 8 bad
        // faces, 7 with a boundary point, 1 without (the interior smoothers,
        // unable to unfold the side, folded an internal face beside it).
        assert!(at_bnd_a >= 1);

        // (B) one global iteration's non-shrinking boundary moves.
        let mut b = m0.clone();
        let cb = UntangleControls { max_global_iterations: 2, boundary_moves: true, ..UntangleControls::default() };
        let rb = untangle_mesh_fv_with(&mut b, &locked, &cb);
        println!("(B) 2 global, non-shrinking: {bad0} -> {} bad faces; {rb:?}", count(&find_bad_faces(&b)));
        assert!(rb.surface_iterations >= 1, "the boundary loop ran");
        let mut worst = 0.0f64;
        for p in 0..m0.points.len() {
            let q = m0.points[p];
            if !on_bnd[p] {
                continue;
            }
            // Distance from each side plane the point started on.
            for (c0, c1) in [(q.x, b.points[p].x), (q.y, b.points[p].y), (q.z, b.points[p].z)] {
                if c0 == 0.0 || c0 == 1.0 {
                    worst = worst.max((c1 - c0).abs());
                }
            }
        }
        println!("(B) largest departure of a flat-side point from its plane: {worst:.3e}");
        assert!(worst <= 1e-12, "{worst}");

        // (C) upstream's defaults with the boundary loop.
        let mut c = m0.clone();
        let cc = UntangleControls { boundary_moves: true, ..UntangleControls::default() };
        let rc = untangle_mesh_fv_with(&mut c, &locked, &cc);
        let left_c = count(&find_bad_faces(&c));
        println!("(C) 10 global with boundary moves: {bad0} -> {left_c} bad faces; dent at {:?}; {rc:?}", c.points[dent]);
        assert_eq!(left_c, 0, "{rc:?}");
        for p in 0..m0.points.len() {
            let q = m0.points[p];
            if (q.x == 0.0 || q.x == 1.0) && (q.y == 0.0 || q.y == 1.0) && (q.z == 0.0 || q.z == 1.0) {
                assert_eq!(c.points[p], q, "corner {p} moved");
            }
        }
        assert_eq!(check_quality(&c).n_negative_volume_cells, 0);
        let mut c2 = m0.clone();
        let _ = untangle_mesh_fv_with(&mut c2, &locked, &cc);
        assert_eq!(c2.points, c.points);
    }

    /// V&V (gh:#1255) — **the `(mesh, locked, nLayers)` region and
    /// `optimizeMeshNearBoundaries`.**
    ///
    /// Methodology: the 8×8×8 unit box (h = 0.125), point (1, 4, 4) (in
    /// cell-index units: one layer in from x = 0) moved by 0.3 h along y,
    /// and the centre point (4, 4, 4) moved by 0.3 h along y.
    ///
    /// Predictions (written 2026-11-05, before the first run):
    /// - [`PartTetMesh::near_boundary`] with 2 layers marks the cells within
    ///   3 of the boundary (index 0–2 or 5–7 on some axis); a point may move
    ///   when every cell around it is marked, i.e. an interior point with
    ///   some index ≤ 2 or ≥ 6: 7³ − 3³ = **316** smooth nodes.
    /// - `optimize_mesh_near_boundaries(2, 2)` moves the near-wall point
    ///   back by more than half its displacement (the volume optimiser
    ///   re-centres a regular star to 1e-4, `volume_optimizer_recentres_a_
    ///   regular_star`; two passes, its neighbours also move a little), leaves
    ///   the centre point bit-identical (not in the region) and every
    ///   boundary point bit-identical (`BOUNDARY`, not `SMOOTH`).
    #[test]
    fn optimize_mesh_near_boundaries_moves_only_the_near_wall_region() {
        let m0 = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [8, 8, 8]);
        let h = 0.125;
        let locked = vec![false; m0.points.len()];
        let tm = PartTetMesh::near_boundary(&m0, &locked, 2);
        assert_eq!(tm.n_smooth_nodes(), 316);
        let at = |m: &VolumeMesh, q: Vec3| m.points.iter().position(|p| p.sub(q).length() < 1e-12).unwrap();
        let near = at(&m0, Vec3::new(h, 0.5, 0.5));
        let centre = at(&m0, Vec3::new(0.5, 0.5, 0.5));
        let mut m = m0.clone();
        m.points[near] = m.points[near].add(Vec3::new(0.0, 0.3 * h, 0.0));
        m.points[centre] = m.points[centre].add(Vec3::new(0.0, 0.3 * h, 0.0));
        let before = m.clone();
        let r = optimize_mesh_near_boundaries(&mut m, &locked, 2, 2);
        let back = m.points[near].sub(m0.points[near]).length();
        println!("near-wall point: 0.3 h -> {:.4} h; {r:?}", back / h);
        assert!(back < 0.15 * h, "{}", back / h);
        assert_eq!(m.points[centre], before.points[centre]);
        for f in m.n_internal_faces()..m.face_count() {
            for &p in &m.faces[f] {
                assert_eq!(m.points[p], before.points[p]);
            }
        }
    }

    /// V&V (gh:#1255) — **`findWorstQualityFaces` and
    /// `optimizeMeshFVBestQuality`.**
    ///
    /// Methodology: the 4×4×4 unit box, as is and with its centre point moved
    /// by (0.1, 0.05, 0.08).
    ///
    /// Predictions (written 2026-11-05, before the first run):
    /// - ~~On the unperturbed box every dot product is 1 and every skewness 0,
    ///   so the thresholds are 1 and 0 and nothing is strictly beyond them:
    ///   0 faces selected at any threshold.~~ **MISSED (first run,
    ///   2026-11-05):** 6 faces were selected at 0.1. The boundary skewness is
    ///   round-off (|d − (n·d) n| about 1e-17, not 0), and the threshold is
    ///   relative to the maximum, so the round-off maximum is selected;
    ///   upstream's formula does the same. The check is now: every face
    ///   selected on the unperturbed box has skewness below 1e-12 and dot
    ///   product above 1 − 1e-12 (round-off only).
    /// - On the perturbed box at threshold 0.1, the face of least dot
    ///   product and the face of greatest skewness are selected, and only
    ///   faces of the 8 cells around the moved point are (no other face is
    ///   non-orthogonal or skewed).
    /// - `optimize_mesh_fv_best_quality(50, 0.1)` lowers the maximum
    ///   non-orthogonality and leaves the boundary bit-identical.
    #[test]
    fn worst_quality_faces_and_best_quality_optimiser() {
        let m0 = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [4, 4, 4]);
        let all = vec![true; m0.face_count()];
        let (d0, s0) = (face_dot_products(&m0, &all), face_skewness(&m0, &all));
        for t in [0.0, 0.1, 0.5, 1.0] {
            let w = find_worst_quality_faces(&m0, &all, t);
            println!("unperturbed box, threshold {t}: {} faces selected", count(&w));
            for f in 0..m0.face_count() {
                if w[f] {
                    assert!(s0[f] < 1e-12 && d0[f] > 1.0 - 1e-12, "threshold {t}, face {f}: skew {} dot {}", s0[f], d0[f]);
                }
            }
        }
        let mut m = m0.clone();
        let c = m.points.iter().position(|p| p.sub(Vec3::new(0.5, 0.5, 0.5)).length() < 1e-12).unwrap();
        m.points[c] = m.points[c].add(Vec3::new(0.1, 0.05, 0.08));
        let worst = find_worst_quality_faces(&m, &all, 0.1);
        let dots = face_dot_products(&m, &all);
        let skew = face_skewness(&m, &all);
        let f_dot = (0..m.face_count()).min_by(|&a, &b| dots[a].partial_cmp(&dots[b]).unwrap()).unwrap();
        let f_skew = (0..m.face_count()).max_by(|&a, &b| skew[a].partial_cmp(&skew[b]).unwrap()).unwrap();
        assert!(worst[f_dot] && worst[f_skew]);
        let pc = point_cells(&m);
        for f in 0..m.face_count() {
            if worst[f] {
                let o = m.owner[f];
                assert!(pc[c].contains(&o) || m.neighbour[f].is_some_and(|n| pc[c].contains(&n)), "face {f}");
            }
        }
        println!("selected {} faces", count(&worst));
        let q0 = check_quality(&m).max_non_orthogonality_deg;
        let locked = vec![false; m.points.len()];
        let before = m.clone();
        let r = optimize_mesh_fv_best_quality(&mut m, &locked, 50, 0.1);
        let q = check_quality(&m).max_non_orthogonality_deg;
        println!("max non-orthogonality {q0:.3} -> {q:.3} deg; {r:?}");
        assert!(q < q0);
        for f in m.n_internal_faces()..m.face_count() {
            for &p in &m.faces[f] {
                assert_eq!(m.points[p], before.points[p]);
            }
        }
    }

    /// V&V (gh:#1256) — **`laplacianPC` on a uniform box is a fixed point,
    /// and one pass is exactly the mean of the point-cells' centres.**
    ///
    /// Methodology: the 4×4×4 unit box; then the same box with the centre
    /// point moved by 0.3 h (h = 0.25) along x.
    ///
    /// Predictions (written 2026-11-05, before the first run):
    /// - Uniform box: every interior point is the mean of its 8 cells'
    ///   centres, so after 5 passes no point has moved more than 1e-12 (only
    ///   round-off of the centroid sums).
    /// - Perturbed box, one pass: the moved point lands **bit for bit** on
    ///   the mean of its 8 cells' centres taken before the pass (the pass
    ///   reads the centres once, `updateMeshGeometry` after); its 26
    ///   neighbours move too (each reads one perturbed cell centre);
    ///   boundary points are bit-identical (not `INSIDE`).
    /// - `laplacian` (edge neighbours) also leaves the uniform box fixed to
    ///   1e-12.
    #[test]
    fn laplace_smoother_fixed_point_and_one_pass() {
        let m0 = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [4, 4, 4]);
        let opt = MeshOptimizer::new(&m0);
        assert_eq!(opt.vertex_location.iter().filter(|&&v| v == vertex_type::INSIDE).count(), 27);
        for kind in [LaplaceKind::PointCells, LaplaceKind::WeightedPointCells, LaplaceKind::Laplacian] {
            let mut m = m0.clone();
            opt.optimize_laplacian(&mut m, kind, 5);
            let worst = m.points.iter().zip(&m0.points).map(|(a, b)| a.sub(*b).length()).fold(0.0, f64::max);
            println!("{kind:?}: uniform box, largest move after 5 passes {worst:.2e}");
            assert!(worst <= 1e-12, "{kind:?} {worst}");
        }
        let mut m = m0.clone();
        let c = m.points.iter().position(|p| p.sub(Vec3::new(0.5, 0.5, 0.5)).length() < 1e-12).unwrap();
        m.points[c] = m.points[c].add(Vec3::new(0.075, 0.0, 0.0));
        // gh:#1385: the centres are cfMesh's `makeCellCentresAndVols` (the
        // earlier expectation used `cell_geometry`, a different pyramid
        // decomposition whose centre differs by 1.3e-5 on this mesh).
        let (_, centres) = crate::checks::foam_cell_centres_and_vols(&m);
        let pc = point_cells(&m);
        let mut expect = Vec3::ZERO;
        for &cl in &pc[c] {
            expect = expect.add(centres[cl]);
        }
        let expect = expect.div(pc[c].len() as f64);
        let before = m.clone();
        opt.optimize_laplacian(&mut m, LaplaceKind::PointCells, 1);
        println!("one pass: x {} -> {} (expected {})", before.points[c].x, m.points[c].x, expect.x);
        assert_eq!(m.points[c], expect);
        let moved = (0..m.points.len()).filter(|&p| m.points[p] != before.points[p]).count();
        assert_eq!(moved, 27, "the centre and its 26 neighbours");
        for f in m.n_internal_faces()..m.face_count() {
            for &p in &m.faces[f] {
                assert_eq!(m.points[p], before.points[p]);
            }
        }
    }

    /// V&V (gh:#1256) — **the lock API.**
    ///
    /// Methodology: [`dented_box`] (8×8×8 box, boundary point dented to
    /// z = 0.2).
    ///
    /// Predictions (written 2026-11-05, before the first run):
    /// - `lock_cells` on one interior cell locks its 6 faces and its 8
    ///   points, and `optimize_mesh_fv` (laplacianPC 5, untangle 10/50/2
    ///   with boundary moves) leaves those 8 points bit-identical while the
    ///   rest of the mesh moves; `remove_user_constraints` clears every lock.
    /// - With `enforce_constraints`, the same untangle stops at the first
    ///   boundary pass with a [`ConstraintViolation`] whose points include
    ///   the dented point, and no boundary point has moved (upstream throws
    ///   before any boundary move).
    #[test]
    fn lock_api_and_enforce_constraints() {
        let (m0, dent) = dented_box();
        let mut opt = MeshOptimizer::new(&m0);
        let cell = (0..m0.n_cells).find(|&cl| {
            let (_, centres) = cell_geometry(&m0);
            centres[cl].sub(Vec3::new(0.3125, 0.3125, 0.5625)).length() < 1e-9
        });
        let cell = cell.expect("an interior cell");
        opt.lock_cells(&m0, &[cell]);
        assert_eq!(opt.locked_faces.len(), 6);
        let locked: Vec<usize> = (0..m0.points.len()).filter(|&p| opt.vertex_location[p] & vertex_type::LOCKED != 0).collect();
        assert_eq!(locked.len(), 8);
        let mut m = m0.clone();
        let controls = UntangleControls { boundary_moves: true, ..UntangleControls::default() };
        let r = opt.optimize_mesh_fv(&mut m, 5, &controls).expect("no constraint enforced");
        println!("optimize_mesh_fv with one locked cell: {r:?}");
        for &p in &locked {
            assert_eq!(m.points[p], m0.points[p]);
        }
        assert!((0..m.points.len()).any(|p| m.points[p] != m0.points[p]));
        opt.remove_user_constraints();
        assert!(opt.locked_faces.is_empty() && opt.locked_points().iter().all(|&l| !l));

        let mut opt = MeshOptimizer::new(&m0);
        opt.set_enforce_constraints("badPoints");
        let mut m = m0.clone();
        let err = opt.untangle_mesh_fv(&mut m, &controls).expect_err("the dent needs a boundary move");
        println!("enforced: {} points in '{}'; {:?}", err.points.len(), err.subset_name, err.report);
        assert!(err.points.contains(&dent));
        for f in m.n_internal_faces()..m.face_count() {
            for &p in &m.faces[f] {
                assert_eq!(m.points[p], m0.points[p]);
            }
        }
    }

    /// Differential (gh:#1256), `--ignored`: cfMesh's `laplacianPC` against
    /// this crate's smart Laplacian ([`crate::smooth::laplacian_smooth`]) on
    /// the graded sphere (R 3, cell 1.0, one refinement level within 1.0 of
    /// the wall: the README's graded case). Five passes each.
    ///
    /// Predictions (written 2026-11-05, before the run): they are different
    /// smoothers (point-cell centres, unconditional; edge neighbours, a move
    /// kept only if no cell inverts), so no quantity should agree exactly.
    /// The smart Laplacian keeps 0 inverted cells by construction;
    /// `laplacianPC` may invert some (upstream untangles after it) but on a
    /// mesh with no fold it should not, so predict 0 for both. Max
    /// non-orthogonality: both lower than the unsmoothed mesh. Mean point
    /// displacement: of the same order (within a factor of 3).
    ///
    /// Results (2026-11-05; 1 381 cells, 3 791 inside points): unsmoothed,
    /// max non-orthogonality 83.80°, max skewness 0.571, 336 bad faces;
    /// `laplacianPC` ×5: 59.44°, 0.366, 0 bad faces, 0 inverted cells, mean
    /// displacement 0.1114 (max 0.2396); smart Laplacian ×5: 73.68°, 0.464,
    /// 0 bad faces, 0 inverted cells, mean 0.0758 (max 0.3518). Every
    /// prediction held.
    #[test]
    #[ignore]
    fn diagnose_laplace_pc_against_smart_laplacian() {
        use crate::pipeline::{sphere_tet_dual, TetDualOptions};
        let o = TetDualOptions { cell_size: 1.0, refinement_levels: 1, refinement_band: 1.0, n_layers: 0, ..TetDualOptions::default() };
        let (m0, _) = sphere_tet_dual(Vec3::ZERO, 3.0, 24, 48, &o).expect("graded sphere");
        let q0 = check_quality(&m0);
        let opt = MeshOptimizer::new(&m0);
        let mut a = m0.clone();
        let t = std::time::Instant::now();
        opt.optimize_laplacian(&mut a, LaplaceKind::PointCells, 5);
        let ta = t.elapsed().as_secs_f64();
        let t = std::time::Instant::now();
        let b = crate::smooth::laplacian_smooth(&m0, 5);
        let tb = t.elapsed().as_secs_f64();
        let disp = |m: &VolumeMesh| {
            let n = opt.vertex_location.iter().filter(|&&v| v & vertex_type::INSIDE != 0).count();
            let s: f64 = (0..m.points.len()).map(|p| m.points[p].sub(m0.points[p]).length()).sum();
            (s / n as f64, (0..m.points.len()).map(|p| m.points[p].sub(m0.points[p]).length()).fold(0.0, f64::max))
        };
        let (qa, qb) = (check_quality(&a), check_quality(&b));
        let (da, db) = (disp(&a), disp(&b));
        println!("graded sphere: {} cells, {} inside points", m0.n_cells, opt.vertex_location.iter().filter(|&&v| v & vertex_type::INSIDE != 0).count());
        println!("unsmoothed:      NO max {:.2}, skew {:.3}, neg cells {}, bad faces {}", q0.max_non_orthogonality_deg, q0.max_skewness, q0.n_negative_volume_cells, count(&find_bad_faces(&m0)));
        println!("laplacianPC x5:  NO max {:.2}, skew {:.3}, neg cells {}, bad faces {}, mean/max disp {:.4}/{:.4}, {ta:.2} s", qa.max_non_orthogonality_deg, qa.max_skewness, qa.n_negative_volume_cells, count(&find_bad_faces(&a)), da.0, da.1);
        println!("smart Laplace x5: NO max {:.2}, skew {:.3}, neg cells {}, bad faces {}, mean/max disp {:.4}/{:.4}, {tb:.2} s", qb.max_non_orthogonality_deg, qb.max_skewness, qb.n_negative_volume_cells, count(&find_bad_faces(&b)), db.0, db.1);
    }

    /// Differential (gh:#1260) — **the three per-point smoothers on the same
    /// simplices.** Methodology: the octahedral star of the tests above with
    /// the free point at (1.4, 0, 0) (4 of 8 tetrahedra inverted), and the
    /// same star with its six outer points jittered and the free point at
    /// (−0.9, 0.7, 0.5); each smoother from the same start.
    ///
    /// Predictions (written 2026-11-05, before the first run):
    /// - every smoother leaves 0 inverted tetrahedra on both stars (Knupp
    ///   by its metric, the LP untangler by construction, the volume
    ///   optimiser because its search starts from the bounding-box centre,
    ///   inside the feasible region of both stars);
    /// - on the symmetric star the LP untangler lands on the origin to 1e-12
    ///   and the volume optimiser to 1e-4; ~~Knupp stops as soon as the point
    ///   is β inside every face, so it is **not** at the origin (more than
    ///   0.1 from it)~~ **MISSED (first run, 2026-11-05):** Knupp lands
    ///   exactly on the origin. The start (1.4, 0, 0) is outside the
    ///   simplex's bounding box, so `knuppMetric` restarts at the box centre,
    ///   where the metric is already zero;
    /// - the smallest tetrahedron volume after each, recorded: LP ≥ Knupp
    ///   (the mean of the feasible region's corners is deeper than "just
    ///   inside"). ~~LP ≥ Knupp~~ **MISSED (2026-11-05):** on the symmetric
    ///   star all three are at the origin (min V 0.1667 each, LP 3e-17 lower
    ///   by round-off); on the jittered star LP 0.1386 < volume optimiser
    ///   0.1418 < Knupp 0.1522. The mean of the region's corners is not the
    ///   point of largest smallest volume, and Knupp, restarting at the box
    ///   centre, happened to land deeper. Positivity (the LP guarantee) holds
    ///   for all three; the ordering is only recorded.
    #[test]
    fn three_smoothers_on_the_same_simplices() {
        let ax = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, -1.0)];
        let build = |jitter: bool, start: Vec3| {
            let mut pts: Vec<Vec3> = ax.to_vec();
            pts.push(Vec3::ZERO);
            let mut tets = Vec::new();
            for &(i, j, k) in &[(0, 2, 4), (2, 1, 4), (1, 3, 4), (3, 0, 4), (2, 0, 5), (1, 2, 5), (3, 1, 5), (0, 3, 5)] {
                let mut t = [i, j, k, 6];
                if tet_volume(pts[t[0]], pts[t[1]], pts[t[2]], pts[6]) < 0.0 {
                    t.swap(0, 1);
                }
                tets.push(t);
            }
            if jitter {
                let j = [[0.1, -0.2, 0.05], [0.15, 0.1, -0.1], [-0.05, 0.2, 0.1], [0.2, -0.1, 0.0], [-0.1, 0.05, -0.2], [0.0, 0.15, 0.1]];
                for (p, d) in pts.iter_mut().zip(j) {
                    *p = p.add(Vec3::new(d[0], d[1], d[2]));
                }
            }
            pts[6] = start;
            (pts, tets)
        };
        let min_vol = |pts: &[Vec3], tets: &[[usize; 4]]| tets.iter().map(|t| tet_volume(pts[t[0]], pts[t[1]], pts[t[2]], pts[t[3]])).fold(f64::MAX, f64::min);
        for (jitter, start) in [(false, Vec3::new(1.4, 0.0, 0.0)), (true, Vec3::new(-0.9, 0.7, 0.5))] {
            let (p0, tets) = build(jitter, start);
            let inv0 = tets.iter().filter(|t| tet_volume(p0[t[0]], p0[t[1]], p0[t[2]], p0[t[3]]) <= 0.0).count();
            let mut k = p0.clone();
            knupp_metric(&mut k, &tets, 6);
            let mut l = p0.clone();
            crate::mesh_untangler::mesh_untangler(&mut l, &tets, 6);
            let mut v = p0.clone();
            volume_optimizer(&mut v, &tets, 6, 1e-5);
            let (mk, ml, mv) = (min_vol(&k, &tets), min_vol(&l, &tets), min_vol(&v, &tets));
            println!(
                "star (jitter {jitter}): {inv0} inverted at start; Knupp -> {:?} min V {mk:.4}; LP -> {:?} min V {ml:.4}; volume optimiser -> {:?} min V {mv:.4}",
                k[6], l[6], v[6]
            );
            assert!(mk > 0.0 && ml > 0.0 && mv > 0.0);
            if !jitter {
                assert!(l[6].length() < 1e-12);
                assert!(v[6].length() < 1e-4);
            }
        }
    }

    /// V&V (gh:#1260) — **the untangler pass inside `untangleMeshFV`.**
    /// Methodology: [`dented_box`] (a boundary point folded in its side),
    /// `untangle_mesh_fv_with` at upstream's defaults with boundary moves,
    /// with the LP untangler off and on.
    ///
    /// Predictions (written 2026-11-05, before the first run): both end
    /// with 0 bad faces (the boundary moves do the work the interior
    /// smoothers cannot); the runs differ in point positions (the LP pass
    /// moves the nodes of inverted tetrahedra first); each is
    /// deterministic.
    /// gh:#1284: `optimiseUsingKnuppMetric` makes **one pass**, whatever
    /// `nIterations` is (upstream's repair loop never counts, so its
    /// `if (nNegative == 0) return;` is always taken). Methodology: a
    /// 4 × 4 × 4 box with three interior points pushed through their
    /// neighbours (inverted tetrahedra), the tet mesh around every face;
    /// `optimise_using_knupp_metric` with 1, 5 and 50 iterations from the
    /// same start. Predictions (written 2026-11-05, before the run): the
    /// three give the same points to the bit; inverted tetrahedra are left
    /// after the pass (so the case is one where the pre-gh:#1284 counting
    /// loop would have gone on to a second pass); the pass moved points.
    #[test]
    fn knupp_metric_makes_one_pass_as_upstream() {
        // Three interior points pushed through their neighbours was the case
        // until gh:#1385 round 3; with the part tet mesh on cfMesh's centres
        // one pass repairs it, so it no longer shows the one-pass rule. The
        // candidates push more points; the first that leaves inverted
        // tetrahedra after one pass is used (the test asserts one exists).
        let pushes = [((0.5, 0.5, 0.5), (0.5, 0.5, 0.95)), ((0.25, 0.25, 0.25), (0.7, 0.25, 0.25)), ((0.75, 0.75, 0.25), (0.75, 0.2, 0.25)), ((0.25, 0.75, 0.75), (0.25, 0.2, 0.75)), ((0.75, 0.25, 0.75), (0.2, 0.25, 0.75)), ((0.5, 0.25, 0.5), (0.5, 0.9, 0.5)), ((0.5, 0.75, 0.25), (0.5, 0.1, 0.25))];
        let mut chosen = None;
        for n_push in 3..=pushes.len() {
            let mut m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [4, 4, 4]);
            for &(from, to) in &pushes[..n_push] {
                let p = m.points.iter().position(|q| q.sub(Vec3::new(from.0, from.1, from.2)).length() < 1e-12).unwrap();
                m.points[p] = Vec3::new(to.0, to.1, to.2);
            }
            let locked = vec![false; m.points.len()];
            let all = vec![true; m.face_count()];
            let start = PartTetMesh::around_faces(&m, &locked, &all, 0);
            let mut one = start.clone();
            one.optimise_using_knupp_metric(1);
            if start.n_inverted_tets() > 0 && one.n_inverted_tets() > 0 {
                chosen = Some(start);
                break;
            }
        }
        let start = chosen.expect("a case with inverted tetrahedra left after one Knupp pass");
        let before = start.n_inverted_tets();
        let mut runs = Vec::new();
        for n in [1usize, 5, 50] {
            let mut tm = start.clone();
            tm.optimise_using_knupp_metric(n);
            runs.push((n, tm.n_inverted_tets(), tm.points.clone()));
        }
        println!("inverted tets: {before} before, after one call with n = 1 / 5 / 50: {:?}", runs.iter().map(|r| r.1).collect::<Vec<_>>());
        assert!(before > 0, "the case has inverted tetrahedra");
        assert!(runs[0].2 != start.points, "the pass moved points");
        assert!(runs[0].1 > 0, "inverted tetrahedra left after the pass: a counting loop would go on");
        assert_eq!(runs[0].2, runs[1].2, "n = 5 is one pass");
        assert_eq!(runs[0].2, runs[2].2, "n = 50 is one pass");
    }

    #[test]
    fn untangle_with_and_without_the_lp_untangler() {
        let (m0, _) = dented_box();
        let locked = vec![false; m0.points.len()];
        let mut res = Vec::new();
        for lpu in [false, true] {
            let c = UntangleControls { boundary_moves: true, mesh_untangler: lpu, ..UntangleControls::default() };
            let mut m = m0.clone();
            let t = std::time::Instant::now();
            let r = untangle_mesh_fv_with(&mut m, &locked, &c);
            let secs = t.elapsed().as_secs_f64();
            let left = count(&find_bad_faces(&m));
            println!("LP untangler {lpu}: {} -> {left} bad faces, {} passes, {} global, {secs:.2} s", r.flagged_before, r.iterations, r.global_iterations);
            assert_eq!(left, 0);
            let mut m2 = m0.clone();
            let _ = untangle_mesh_fv_with(&mut m2, &locked, &c);
            assert_eq!(m2.points, m.points);
            res.push(m.points);
        }
        println!("positions differ: {}", res[0] != res[1]);
    }

    /// The subset API on the optimiser (gh:#1261): `lockCellsInSubset`
    /// locks what `lockCells` locks; an unknown name locks nothing and says
    /// so; `untangle_mesh_fv_recording` writes `badPoints` on a constraint
    /// violation. Predictions (written 2026-11-05, before the first run):
    /// the same 8 points locked as by `lock_cells` on that cell; `false` for
    /// a missing subset; the recorded `badPoints` subset holds the folded
    /// point.
    #[test]
    fn subset_locks_and_recorded_subsets() {
        use crate::mesh_subsets::MeshSubsets;
        let (m0, dent) = dented_box();
        let mut subsets = MeshSubsets::default();
        let id = subsets.cells.add("lockMe");
        subsets.cells.add_element(id, 100);
        let mut a = MeshOptimizer::new(&m0);
        assert!(a.lock_cells_in_subset(&m0, &subsets, "lockMe"));
        assert!(!a.lock_cells_in_subset(&m0, &subsets, "missing"));
        let mut b = MeshOptimizer::new(&m0);
        b.lock_cells(&m0, &[100]);
        assert_eq!(a.vertex_location, b.vertex_location);
        assert_eq!(a.locked_points().iter().filter(|&&l| l).count(), 8);
        let pid = subsets.points.add("pts");
        subsets.points.add_element(pid, 3);
        assert!(a.lock_points_in_subset(&subsets, "pts"));
        assert!(a.locked_points()[3]);
        let fid = subsets.faces.add("f");
        subsets.faces.add_element(fid, 0);
        assert!(a.lock_faces_in_subset(&m0, &subsets, "f"));

        let mut opt = MeshOptimizer::new(&m0);
        opt.set_enforce_constraints("badPoints");
        let mut m = m0.clone();
        let c = UntangleControls { boundary_moves: true, ..UntangleControls::default() };
        assert!(opt.untangle_mesh_fv_recording(&mut m, &mut subsets, &c).is_err());
        let bp = subsets.points.index("badPoints").expect("badPoints written");
        assert!(subsets.points.elements(bp).contains(&dent));
        // Without boundary moves the folded side stays bad: badFaces and
        // badCells are written.
        let mut m = m0.clone();
        let opt = MeshOptimizer::new(&m0);
        let r = opt.untangle_mesh_fv_recording(&mut m, &mut subsets, &UntangleControls { max_global_iterations: 2, ..UntangleControls::default() }).unwrap();
        assert!(r.flagged_after > 0 || count(&find_bad_faces(&m)) > 0);
        let bf = subsets.faces.index("badFaces").expect("badFaces written");
        assert_eq!(subsets.faces.elements(bf).len(), count(&find_bad_faces(&m)));
        assert!(subsets.cells.index("badCells").is_some());
    }
}
