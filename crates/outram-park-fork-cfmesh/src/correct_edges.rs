// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh v1.1 (github.com/DAFoam/cfmesh, commit e944f6ba695f):
//   meshLibrary/utilities/surfaceTools/correctEdgesBetweenPatches/
//     correctEdgesBetweenPatches.C              (the constructor's order)
//     correctEdgesBetweenPatchesDistributeFaces.C (decomposeConcaveFaces,
//                                                  patchCorrection)
//   meshLibrary/utilities/surfaceTools/meshSurfaceCheckEdgeTypes/
//     meshSurfaceCheckEdgeTypes.C               (classifyEdges: the tet test)
//   meshLibrary/utilities/surfaceTools/meshSurfacePartitioner/
//     meshSurfacePartitionerFunctions.C         (corners: > 2 feature edges,
//                                                  edge points: exactly 2)
//   meshLibrary/utilities/faceDecomposition/decomposeFaces.C (the fan)
//   meshLibrary/utilities/decomposeCells/
//     decomposeCellsDecomposition.C, decomposeCellsPyramids.C
//     (checkFaceConnections, findTopVertex, decomposeCellIntoPyramids)
// Copyright (C) 2014-2017 Creative Fields, Ltd. Licence: GPL-3.0-only.
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

//! **`correctEdgesBetweenPatches`** (gh:#1241): cfMesh's topological repair
//! of the boundary before the feature-edge capture
//! ([`crate::surface_mapper`]). Upstream runs it from
//! `meshSurfaceEdgeExtractorNonTopo::decomposeBoundaryFaces`, after the
//! patches are known and before `mapVerticesOntoSurfacePatches`. It moves no
//! point; it splits faces and cells so that the capture cannot fold them:
//!
//! 1. `decomposeConcaveFaces`: an internal face touching a concave boundary
//!    point, with boundary edges in two or more patches (a feature edge
//!    counts as patch `-1`, as upstream's `edgeInPatch`), is split into a
//!    triangle fan about its centre; both its cells are marked.
//! 2. `patchCorrection`: a boundary face with a vertex that is an edge point
//!    (exactly two feature edges) whose two edges in the face are both
//!    feature edges is split, a quad into two triangles at that vertex, a
//!    larger face into a fan; its owner is marked. Triangles are kept.
//! 3. Every marked cell is split into pyramids, one per face, apex at the
//!    average of its vertices, after fanning each face that meets another
//!    face of the cell on more than one edge (`checkFaceConnections`).
//!
//! Upstream's third step, `decomposeProblematicFaces`, is commented out in
//! its constructor and is not ported. Not ported either:
//! `meshSurfaceCheckInvertedVertices` (upstream marks an edge at a tangled
//! point `UNDETERMINED` and treats it as concave); here only the tet test
//! classifies an edge. A new fan point is placed at OpenFOAM's
//! `face::centre` (area-weighted), as upstream.
//!
//! **UPDATED 2026-11-05 (gh:#1257):** the boundary addressing (edges, the
//! faces at each edge, feature edges, edge points) comes from
//! [`crate::surface_engine`], not from maps built here. With it the tet test
//! follows `classifyEdges` exactly: `f0` is the engine's first face of the
//! edge (the one holding it start -> end), and the edge is concave when
//! `tet(s, e, c0, c1)` **or** `tet(e, s, c1, c0)` is not below `-VSMALL`
//! (before: the lower-numbered face and the first tet only; the two tets are
//! equal analytically, so this can differ from before only at round-off).
//!
//! The patch of a boundary face is given by the caller, per face, and the
//! children of a split face inherit it (`newBoundaryPatches_`); the capture
//! must then use the same assignment, not re-derive it from the children's
//! centroids ([`crate::surface_mapper::map_vertices_onto_surface_patches_untangled_given`]).

use crate::math::Vec3;
use crate::surface_engine::{NOT_BOUNDARY, VSMALL};
use crate::topology::{decompose_cells, TopoMesh};
use crate::volume_mesh::{BoundaryPatch, VolumeMesh};
use std::collections::{BTreeMap, BTreeSet};

/// What [`correct_edges_between_patches`] did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CorrectEdgesReport {
    /// Internal faces fanned by `decomposeConcaveFaces`.
    pub n_concave_faces: usize,
    /// Boundary faces split by `patchCorrection`.
    pub n_patch_corrected_faces: usize,
    /// Faces fanned by `checkFaceConnections` before the pyramids.
    pub n_connection_faces: usize,
    /// Cells split into pyramids.
    pub n_decomposed_cells: usize,
}

/// OpenFOAM `face::centre` (OpenFOAM-v2506 `face.C`), operation for
/// operation: a triangle is `(1.0/3.0)*(p0 + p1 + p2)`; any other face the
/// area-weighted centroid of the fan about the vertex average (`centrePoint
/// /= nPoints`), with `2*area = |(p - c) x (next - c)|` and the result
/// `sumAc/(3.0*sumA)` (each component divided). gh:#1385: the earlier port
/// used `(q - p) x (avg - p)` and multiplied by reciprocals, which differ
/// from upstream in the last bit.
pub fn foam_face_centre(points: &[Vec3], ring: &[usize]) -> Vec3 {
    let k = ring.len();
    if k == 3 {
        return points[ring[0]].add(points[ring[1]]).add(points[ring[2]]).scale(1.0 / 3.0);
    }
    let avg = ring.iter().fold(Vec3::ZERO, |a, &v| a.add(points[v])).div(k as f64);
    let (mut sum_ac, mut sum_a) = (Vec3::ZERO, 0.0);
    for i in 0..k {
        let (p, q) = (points[ring[i]], points[ring[(i + 1) % k]]);
        let ttc = p.add(q).add(avg);
        let ta = p.sub(avg).cross(q.sub(avg)).length();
        sum_a += ta;
        sum_ac = sum_ac.add(ttc.scale(ta));
    }
    if sum_a > 1e-300 {
        sum_ac.div(3.0 * sum_a)
    } else {
        avg
    }
}



/// Run cfMesh's `correctEdgesBetweenPatches` on `mesh`, whose boundary face
/// `f` lies in patch `face_region[f]` (`None` for an internal face). Returns
/// the repaired mesh, its per-face patch, and what was done. Boundary faces
/// are ordered by patch, internal faces first; `names[r]` names patch `r`.
/// See the module documentation for the steps and what is not ported.
pub fn correct_edges_between_patches(
    mesh: &VolumeMesh,
    face_region: &[Option<usize>],
    names: &[String],
) -> (VolumeMesh, Vec<Option<usize>>, CorrectEdgesReport) {
    let mut w = TopoMesh {
        points: mesh.points.clone(),
        faces: mesh.faces.clone(),
        owner: mesh.owner.clone(),
        neighbour: mesh.neighbour.clone(),
        region: face_region.to_vec(),
        n_cells: mesh.n_cells,
        patch_names: names.to_vec(),
        cells: if mesh.carried_cells_are_consistent() { mesh.cells.clone() } else { None },
    };
    let mut rep = CorrectEdgesReport::default();
    let mut decompose_cell = vec![false; mesh.n_cells];

    // 1. decomposeConcaveFaces.
    {
        let (engine, part) = w.surface();
        let mut concave: BTreeSet<usize> = BTreeSet::new();
        // edgeInPatch: the patch of a non-feature two-face edge, -1 otherwise.
        let mut edge_in_patch: Vec<i64> = vec![-1; engine.edges.len()];
        for (e, fs) in engine.edge_faces.iter().enumerate() {
            if fs.len() != 2 {
                continue;
            }
            if !part.feature_edges.contains(&e) {
                edge_in_patch[e] = w.region[engine.boundary_faces[fs[0]]].map_or(-1, |r| r as i64);
                continue;
            }
            // meshSurfaceCheckEdgeTypes::classifyEdges: f0 holds the edge
            // start -> end (the engine's edgeFaces order); concave when
            // tet(s, e, c0, c1) or tet(e, s, c1, c0) is not negative.
            let [s, t] = engine.edges[e];
            let (c0, c1) = (engine.face_centres[fs[0]], engine.face_centres[fs[1]]);
            let (ps, pt) = (w.points[s], w.points[t]);
            let vol0 = pt.sub(ps).cross(c0.sub(ps)).dot(c1.sub(ps)) / 6.0;
            let vol1 = ps.sub(pt).cross(c1.sub(pt)).dot(c0.sub(pt)) / 6.0;
            if vol0 > -VSMALL || vol1 > -VSMALL {
                concave.insert(s);
                concave.insert(t);
            }
        }
        let mut split = BTreeMap::new();
        for f in 0..w.faces.len() {
            let Some(nb) = w.neighbour[f] else { continue };
            let ring = &w.faces[f];
            let k = ring.len();
            let mut has_concave = false;
            let mut n_bnd = 0usize;
            let mut patches: Vec<i64> = Vec::new();
            for i in 0..k {
                if engine.bp[ring[i]] == NOT_BOUNDARY {
                    continue;
                }
                if concave.contains(&ring[i]) {
                    has_concave = true;
                }
                if let Some(e) = engine.find_edge(ring[i], ring[(i + 1) % k]) {
                    let p = edge_in_patch[e];
                    n_bnd += 1;
                    if !patches.contains(&p) {
                        patches.push(p);
                    }
                }
            }
            if has_concave && n_bnd > 1 && patches.len() > 1 {
                decompose_cell[w.owner[f]] = true;
                decompose_cell[nb] = true;
                let fan = w.fan(f);
                split.insert(f, fan);
            }
        }
        rep.n_concave_faces = split.len();
        w.apply_splits(&split);
    }

    // 2. patchCorrection.
    {
        let (engine, part) = w.surface();
        let is_f = |a: usize, b: usize| engine.find_edge(a, b).is_some_and(|e| part.feature_edges.contains(&e));
        let mut split = BTreeMap::new();
        for f in 0..w.faces.len() {
            if w.neighbour[f].is_some() {
                continue;
            }
            let bf = w.faces[f].clone();
            let k = bf.len();
            for i in 0..k {
                let edge_point = part.edge_points.contains(&engine.bp[bf[i]]);
                if edge_point && is_f(bf[i], bf[(i + 1) % k]) && is_f(bf[(i + k - 1) % k], bf[i]) {
                    if k > 4 {
                        let fan = w.fan(f);
                        split.insert(f, fan);
                    } else if k == 4 {
                        split.insert(f, vec![vec![bf[i], bf[(i + 1) % 4], bf[(i + 2) % 4]], vec![bf[i], bf[(i + 2) % 4], bf[(i + 3) % 4]]]);
                    } else {
                        continue;
                    }
                    decompose_cell[w.owner[f]] = true;
                    break;
                }
            }
        }
        rep.n_patch_corrected_faces = split.len();
        w.apply_splits(&split);
    }

    // 3. decomposeCells: checkFaceConnections, then pyramids.
    if decompose_cell.iter().any(|&d| d) {
        rep.n_connection_faces = decompose_cells(&mut w, &decompose_cell);
        rep.n_decomposed_cells = decompose_cell.iter().filter(|&&d| d).count();
    }

    // Order: internal faces first, then each patch contiguous.
    let mut order: Vec<usize> = (0..w.faces.len()).filter(|&f| w.neighbour[f].is_some()).collect();
    let mut patches = Vec::new();
    for (r, name) in names.iter().enumerate() {
        let start = order.len();
        order.extend((0..w.faces.len()).filter(|&f| w.neighbour[f].is_none() && w.region[f] == Some(r)));
        if order.len() > start {
            patches.push(BoundaryPatch { name: name.clone(), start_face: start, n_faces: order.len() - start });
        }
    }
    let start = order.len();
    order.extend((0..w.faces.len()).filter(|&f| w.neighbour[f].is_none() && w.region[f].is_none_or(|r| r >= names.len())));
    if order.len() > start {
        patches.push(BoundaryPatch { name: "unassigned".into(), start_face: start, n_faces: order.len() - start });
    }
    // the cell lists survive only while nothing changed the cells or their
    // faces (gh:#1385); the faces' new labels follow `order`
    let cells = match &w.cells {
        Some(c) if crate::volume_mesh::cell_lists_are_consistent(c, w.n_cells, &w.owner, &w.neighbour) => {
            let mut new_label = vec![usize::MAX; w.faces.len()];
            for (n, &o) in order.iter().enumerate() {
                new_label[o] = n;
            }
            Some(c.iter().map(|l| l.iter().map(|&f| new_label[f]).collect()).collect())
        }
        _ => None,
    };
    let out = VolumeMesh {
        points: w.points,
        faces: order.iter().map(|&f| w.faces[f].clone()).collect(),
        owner: order.iter().map(|&f| w.owner[f]).collect(),
        neighbour: order.iter().map(|&f| w.neighbour[f]).collect(),
        n_cells: w.n_cells,
        patches,
        cells,
    };
    let regions = order.iter().map(|&f| w.region[f]).collect();
    (out, regions, rep)
}

/// `decomposeCells::decomposeMesh` and the bare-mesh form the octree track
/// uses: one shared copy in [`crate::topology`] (gh:#1267 moved it there
/// unchanged; `cartesian_mesh` reaches it through this path).
pub(crate) use crate::topology::decompose_faces_and_cells;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;
    use crate::topology::decompose_cells_into_pyramids;

    /// A flat triangle's centre is its vertex average; a square's too.
    #[test]
    fn face_centre_matches_openfoam_on_simple_faces() {
        let p = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 0.0)];
        let c = foam_face_centre(&p, &[0, 1, 2, 3]);
        assert!((c.x - 0.5).abs() < 1e-15 && (c.y - 0.5).abs() < 1e-15);
        let t = foam_face_centre(&p, &[0, 1, 2]);
        assert!((t.x - 2.0 / 3.0).abs() < 1e-15 && (t.y - 1.0 / 3.0).abs() < 1e-15);
    }

    /// Pyramid decomposition of every cell of a 2x2x2 box keeps the mesh
    /// closed and the volume, and gives 6 pyramids per hexahedron; a box
    /// whose patches meet only at convex 90-degree edges has no concave face
    /// and no face with two feature edges at an edge point, so nothing
    /// changes.
    #[test]
    fn a_box_is_left_alone_and_pyramids_conserve_volume() {
        let m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [2, 2, 2]);
        // One patch per box side, by the face normal.
        let region: Vec<Option<usize>> = (0..m.face_count())
            .map(|f| {
                m.neighbour[f].is_none().then(|| {
                    let s = m.face_area_vector(f);
                    let ax = [s.x.abs(), s.y.abs(), s.z.abs()];
                    let i = (0..3).max_by(|&a, &b| ax[a].partial_cmp(&ax[b]).unwrap()).unwrap();
                    2 * i + usize::from([s.x, s.y, s.z][i] > 0.0)
                })
            })
            .collect();
        let names: Vec<String> = (0..6).map(|i| format!("side{i}")).collect();
        let (out, _, rep) = correct_edges_between_patches(&m, &region, &names);
        assert_eq!(rep, CorrectEdgesReport::default());
        assert_eq!(out.cell_count(), 8);
        assert!((out.total_volume() - 1.0).abs() < 1e-12);

        let mut w = TopoMesh { points: m.points.clone(), faces: m.faces.clone(), owner: m.owner.clone(), neighbour: m.neighbour.clone(), region, n_cells: 8, patch_names: Vec::new(), cells: None };
        decompose_cells_into_pyramids(&mut w, &[true; 8]);
        let v = VolumeMesh { points: w.points, faces: w.faces, owner: w.owner, neighbour: w.neighbour, n_cells: w.n_cells, patches: vec![] , cells: None };
        assert_eq!(v.cell_count(), 48);
        v.validate().unwrap();
        assert!((v.total_volume() - 1.0).abs() < 1e-12);
        assert!(v.owner.iter().zip(&v.neighbour).all(|(&o, n)| n.is_none_or(|n| o < n)));
        assert_eq!(crate::checks::check_quality(&v).n_negative_volume_cells, 0);
    }

    /// Pin (gh:#1267): `correct_edges_between_patches` bit for bit, on the
    /// mesh the pipeline hands it. Methodology: the demo pipe shortened to
    /// L 60 mm (R 10 mm, 48 facets), h 6 mm, `MedianPerPoint` with
    /// `Corners`, bare; pre-mapped and given its patches as the pipeline
    /// does (`surface_to_tet_dual_mesh_multipatch` with `correct_edges`),
    /// then corrected. The fingerprint covers the mesh (points bit for bit,
    /// rings, owner, neighbour, patches), the per-face patch and the report.
    /// Recorded 2026-11-05 **before** the pyramid decomposition moved into
    /// [`crate::topology`], and asserted after: the move must not change a
    /// bit (prediction: it does not, the code is moved unchanged).
    #[test]
    fn correct_edges_is_pinned_bit_for_bit() {
        use crate::dual::{DualKind, FeaturePointRule};
        use crate::pipeline::tests::{mesh_fingerprint, pipe_surface};
        use crate::pipeline::{surface_to_tet_dual_mesh_multipatch, TetDualOptions};
        let (p, t, regions) = pipe_surface(0.01, 0.06, 48);
        let o = TetDualOptions { cell_size: 0.006, n_layers: 0, dual_kind: DualKind::MedianPerPoint, dual_feature_points: FeaturePointRule::Corners, ..Default::default() };
        let (bare, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
        let (pre, _) = crate::surface_mapper::map_vertices_onto_surface_untangled(&bare, &p, &t, 10);
        let given = crate::surface_mapper::boundary_face_regions(&pre, &p, &t, &regions);
        let (out, reg, rep) = correct_edges_between_patches(&pre, &given, &regions.names);
        let mut h = mesh_fingerprint(&out);
        for r in &reg {
            h = h.rotate_left(7) ^ r.map_or(u64::MAX, |r| r as u64);
        }
        let got = (out.cell_count(), rep.n_concave_faces, rep.n_patch_corrected_faces, rep.n_connection_faces, rep.n_decomposed_cells, h);
        println!("correct_edges pin {got:?}");
        assert_eq!(got, CORRECT_EDGES_PIN);
    }
    /// gh:#1285: its input is the pipeline's output, now in the
    /// upper-triangular face order; the counts did not move, the hash did
    /// (~~714198749002929189~~).
    ///
    /// **Re-pinned 2026-11-11 (gh:#1385):** ~~7179125119703641544~~
    /// 15811906865840324897. The counts did not move; the hash did because
    /// the pipeline that makes its input now follows upstream bit for bit
    /// where it was compared: `foam_face_centre` and `ring_area_vector` as
    /// `face::centre` / `face::areaNormal`, divisions where OpenFOAM divides,
    /// the octree's nearest-point search in the mapper, the layer's faces in
    /// `addCells` / `reorderBoundaryFaces` order, `meshSurfaceEdgeExtractor
    /// NonTopo` also on a one-region surface. Geometry moved in the last bit;
    /// face order moved.
    const CORRECT_EDGES_PIN: (usize, usize, usize, usize, usize, u64) = (1727, 8, 16, 112, 14, 15811906865840324897);

    /// V&V (gh:#1241) — `patchCorrection` by hand. One unit cube, its top
    /// face in patch 1 and the rest in patch 0: the top's four edges are
    /// feature edges and each top corner has exactly two (an edge point),
    /// both in the top face, so upstream splits the top quad into two
    /// triangles at its first such vertex (`bf[0], bf[1], bf[2]` and
    /// `bf[0], bf[2], bf[3]`) and the cube into pyramids, one per face:
    /// 5 quads + 2 triangles = 7 pyramids about the vertex average
    /// (0.5, 0.5, 0.5). The side faces have one feature edge each and stay.
    /// Predicted before the first run: 1 face split, 1 cell decomposed, 7
    /// cells, volume 1 exactly, closed, no inverted cell, the top patch
    /// two triangles, the side patch five quads.
    #[test]
    fn patch_correction_splits_a_face_with_two_feature_edges_at_an_edge_point() {
        let m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [1, 1, 1]);
        let region: Vec<Option<usize>> =
            (0..m.face_count()).map(|f| m.neighbour[f].is_none().then(|| usize::from(m.face_area_vector(f).z > 0.5))).collect();
        let names = vec!["sides".to_string(), "top".to_string()];
        let (out, reg, rep) = correct_edges_between_patches(&m, &region, &names);
        assert_eq!(rep, CorrectEdgesReport { n_concave_faces: 0, n_patch_corrected_faces: 1, n_connection_faces: 0, n_decomposed_cells: 1 });
        assert_eq!(out.cell_count(), 7);
        out.validate().unwrap();
        assert!((out.total_volume() - 1.0).abs() < 1e-14);
        assert_eq!(crate::checks::check_quality(&out).n_negative_volume_cells, 0);
        let top: Vec<usize> = (0..out.face_count()).filter(|&f| reg[f] == Some(1)).collect();
        assert_eq!(top.len(), 2);
        assert!(top.iter().all(|&f| out.faces[f].len() == 3));
        assert_eq!((0..out.face_count()).filter(|&f| reg[f] == Some(0)).count(), 5);
        assert_eq!(out.patches.iter().map(|p| (p.name.as_str(), p.n_faces)).collect::<Vec<_>>(), vec![("sides", 5), ("top", 2)]);
        // The apex: one new point at the cube's vertex average.
        assert_eq!(out.point_count(), 9);
        let apex = out.points[8];
        assert!((apex.x - 0.5).abs() < 1e-15 && (apex.y - 0.5).abs() < 1e-15 && (apex.z - 0.5).abs() < 1e-15);
    }
}
