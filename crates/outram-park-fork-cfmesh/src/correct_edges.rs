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
//! The patch of a boundary face is given by the caller, per face, and the
//! children of a split face inherit it (`newBoundaryPatches_`); the capture
//! must then use the same assignment, not re-derive it from the children's
//! centroids ([`crate::surface_mapper::map_vertices_onto_surface_patches_untangled_given`]).

use crate::math::Vec3;
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

/// OpenFOAM `face::centre`: the area-weighted centroid of the triangle fan
/// about the vertex average (the vertex average for a triangle).
pub fn foam_face_centre(points: &[Vec3], ring: &[usize]) -> Vec3 {
    let k = ring.len();
    let avg = ring.iter().fold(Vec3::ZERO, |a, &v| a.add(points[v])).scale(1.0 / k as f64);
    if k == 3 {
        return avg;
    }
    let (mut sum_ac, mut sum_a) = (Vec3::ZERO, 0.0);
    for i in 0..k {
        let (p, q) = (points[ring[i]], points[ring[(i + 1) % k]]);
        let c = p.add(q).add(avg);
        let a = q.sub(p).cross(avg.sub(p)).length();
        sum_ac = sum_ac.add(c.scale(a));
        sum_a += a;
    }
    if sum_a > 1e-300 {
        sum_ac.scale(1.0 / (3.0 * sum_a))
    } else {
        avg
    }
}

/// A mesh being repaired: faces with owner, neighbour and (boundary) patch.
struct Work {
    points: Vec<Vec3>,
    faces: Vec<Vec<usize>>,
    owner: Vec<usize>,
    neighbour: Vec<Option<usize>>,
    region: Vec<Option<usize>>,
    n_cells: usize,
}

impl Work {
    /// Replace each face in `split` by its children (same owner, neighbour
    /// and patch; the rings keep the parent's winding).
    fn apply_splits(&mut self, split: &BTreeMap<usize, Vec<Vec<usize>>>) {
        let mut faces = Vec::with_capacity(self.faces.len());
        let (mut owner, mut neighbour, mut region) = (Vec::new(), Vec::new(), Vec::new());
        for f in 0..self.faces.len() {
            let children = match split.get(&f) {
                Some(c) => c.clone(),
                None => vec![self.faces[f].clone()],
            };
            for c in children {
                faces.push(c);
                owner.push(self.owner[f]);
                neighbour.push(self.neighbour[f]);
                region.push(self.region[f]);
            }
        }
        self.faces = faces;
        self.owner = owner;
        self.neighbour = neighbour;
        self.region = region;
    }

    /// `decomposeFaces`: the fan of face `f` about a new point at its centre.
    fn fan(&mut self, f: usize) -> Vec<Vec<usize>> {
        let ring = self.faces[f].clone();
        let c = self.points.len();
        self.points.push(foam_face_centre(&self.points, &ring));
        (0..ring.len()).map(|i| vec![ring[i], ring[(i + 1) % ring.len()], c]).collect()
    }

    /// Boundary edges (sorted pairs) with the boundary faces on them.
    fn boundary_edge_faces(&self) -> BTreeMap<(usize, usize), Vec<usize>> {
        let mut ef: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for f in 0..self.faces.len() {
            if self.neighbour[f].is_none() {
                let ring = &self.faces[f];
                for i in 0..ring.len() {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    ef.entry((a.min(b), a.max(b))).or_default().push(f);
                }
            }
        }
        ef
    }
}

fn is_feature(ef: &[usize], region: &[Option<usize>]) -> bool {
    ef.len() == 2 && region[ef[0]] != region[ef[1]]
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
    let mut w = Work {
        points: mesh.points.clone(),
        faces: mesh.faces.clone(),
        owner: mesh.owner.clone(),
        neighbour: mesh.neighbour.clone(),
        region: face_region.to_vec(),
        n_cells: mesh.n_cells,
    };
    let mut rep = CorrectEdgesReport::default();
    let mut decompose_cell = vec![false; mesh.n_cells];

    // 1. decomposeConcaveFaces.
    {
        let ef = w.boundary_edge_faces();
        let centres: Vec<Option<Vec3>> =
            (0..w.faces.len()).map(|f| w.neighbour[f].is_none().then(|| foam_face_centre(&w.points, &w.faces[f]))).collect();
        let mut concave: BTreeSet<usize> = BTreeSet::new();
        let mut edge_in_patch: BTreeMap<(usize, usize), i64> = BTreeMap::new();
        for (&(a, b), fs) in &ef {
            if fs.len() != 2 {
                edge_in_patch.insert((a, b), -1);
                continue;
            }
            if !is_feature(fs, &w.region) {
                edge_in_patch.insert((a, b), w.region[fs[0]].map_or(-1, |r| r as i64));
                continue;
            }
            edge_in_patch.insert((a, b), -1);
            // meshSurfaceCheckEdgeTypes: f0 is the face holding the edge as
            // s -> e; tet(s, e, c0, c1) not negative is concave.
            let ring0 = &w.faces[fs[0]];
            let k = ring0.len();
            let forward = (0..k).any(|i| ring0[i] == a && ring0[(i + 1) % k] == b);
            let (s, e) = if forward { (a, b) } else { (b, a) };
            let (c0, c1) = (centres[fs[0]].unwrap(), centres[fs[1]].unwrap());
            let (ps, pe) = (w.points[s], w.points[e]);
            let vol = pe.sub(ps).cross(c0.sub(ps)).dot(c1.sub(ps)) / 6.0;
            if vol > -1e-300 {
                concave.insert(s);
                concave.insert(e);
            }
        }
        let bpoints: BTreeSet<usize> = ef.keys().flat_map(|&(a, b)| [a, b]).collect();
        let mut split = BTreeMap::new();
        for f in 0..w.faces.len() {
            let Some(nb) = w.neighbour[f] else { continue };
            let ring = &w.faces[f];
            let k = ring.len();
            let mut has_concave = false;
            let mut n_bnd = 0usize;
            let mut patches: Vec<i64> = Vec::new();
            for i in 0..k {
                if !bpoints.contains(&ring[i]) {
                    continue;
                }
                if concave.contains(&ring[i]) {
                    has_concave = true;
                }
                let (a, b) = (ring[i], ring[(i + 1) % k]);
                if let Some(&p) = edge_in_patch.get(&(a.min(b), a.max(b))) {
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
        let ef = w.boundary_edge_faces();
        let mut n_feat: BTreeMap<usize, usize> = BTreeMap::new();
        let mut feature: BTreeSet<(usize, usize)> = BTreeSet::new();
        for (&e, fs) in &ef {
            if is_feature(fs, &w.region) {
                feature.insert(e);
                *n_feat.entry(e.0).or_default() += 1;
                *n_feat.entry(e.1).or_default() += 1;
            }
        }
        let is_f = |a: usize, b: usize| feature.contains(&(a.min(b), a.max(b)));
        let mut split = BTreeMap::new();
        for f in 0..w.faces.len() {
            if w.neighbour[f].is_some() {
                continue;
            }
            let bf = w.faces[f].clone();
            let k = bf.len();
            for i in 0..k {
                let edge_point = n_feat.get(&bf[i]) == Some(&2);
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
        let mut cell_faces: Vec<Vec<usize>> = vec![Vec::new(); w.n_cells];
        for f in 0..w.faces.len() {
            cell_faces[w.owner[f]].push(f);
            if let Some(n) = w.neighbour[f] {
                cell_faces[n].push(f);
            }
        }
        let mut split = BTreeMap::new();
        for c in (0..w.n_cells).filter(|&c| decompose_cell[c]) {
            let mut edge_faces: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
            for &f in &cell_faces[c] {
                let ring = &w.faces[f];
                for i in 0..ring.len() {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    edge_faces.entry((a.min(b), a.max(b))).or_default().push(f);
                }
            }
            for &f in &cell_faces[c] {
                let ring = w.faces[f].clone();
                let mut nei: BTreeSet<usize> = BTreeSet::new();
                for i in 0..ring.len() {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    let other = edge_faces[&(a.min(b), a.max(b))].iter().copied().find(|&g| g != f).unwrap_or(f);
                    if !nei.insert(other) && !split.contains_key(&f) {
                        let fan = w.fan(f);
                        split.insert(f, fan);
                    }
                }
            }
        }
        rep.n_connection_faces = split.len();
        w.apply_splits(&split);
        pyramids(&mut w, &decompose_cell);
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
    let out = VolumeMesh {
        points: w.points,
        faces: order.iter().map(|&f| w.faces[f].clone()).collect(),
        owner: order.iter().map(|&f| w.owner[f]).collect(),
        neighbour: order.iter().map(|&f| w.neighbour[f]).collect(),
        n_cells: w.n_cells,
        patches,
    };
    let regions = order.iter().map(|&f| w.region[f]).collect();
    (out, regions, rep)
}

/// `decomposeCellIntoPyramids` for every marked cell: one pyramid per face,
/// apex at the average of the cell's vertices (`findTopVertex`). Unmarked
/// cells keep their order (renumbered past the removed ones); the pyramids
/// are appended cell by cell, face by face (`removeDecomposedCells`,
/// `addNewCells`). Internal faces keep owner < neighbour.
fn pyramids(w: &mut Work, mark: &[bool]) {
    let mut new_id = vec![usize::MAX; w.n_cells];
    let mut next = 0usize;
    for c in 0..w.n_cells {
        if !mark[c] {
            new_id[c] = next;
            next += 1;
        }
    }
    // Faces of each marked cell, outward rings.
    let mut cell_faces: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for f in 0..w.faces.len() {
        if mark[w.owner[f]] {
            cell_faces.entry(w.owner[f]).or_default().push(f);
        }
        if let Some(n) = w.neighbour[f] {
            if mark[n] {
                cell_faces.entry(n).or_default().push(f);
            }
        }
    }
    // Pyramid id of (cell, face).
    let mut pyr: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (&c, fs) in &cell_faces {
        for &f in fs {
            pyr.insert((c, f), next);
            next += 1;
        }
    }
    let n_old = w.faces.len();
    let (mut faces, mut owner, mut neighbour, mut region) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut push = |ring: Vec<usize>, o: usize, n: Option<usize>, r: Option<usize>| {
        let (ring, o, n) = match n {
            Some(nb) if nb < o => (ring.into_iter().rev().collect(), nb, Some(o)),
            _ => (ring, o, n),
        };
        faces.push(ring);
        owner.push(o);
        neighbour.push(n);
        region.push(r);
    };
    for f in 0..n_old {
        let o = w.owner[f];
        let o2 = if mark[o] { pyr[&(o, f)] } else { new_id[o] };
        let n2 = w.neighbour[f].map(|n| if mark[n] { pyr[&(n, f)] } else { new_id[n] });
        push(w.faces[f].clone(), o2, n2, w.region[f]);
    }
    for (&c, fs) in &cell_faces {
        let verts: BTreeSet<usize> = fs.iter().flat_map(|&f| w.faces[f].iter().copied()).collect();
        let apex_p = verts.iter().fold(Vec3::ZERO, |a, &v| a.add(w.points[v])).scale(1.0 / verts.len() as f64);
        let apex = w.points.len();
        w.points.push(apex_p);
        // Directed edges of the outward rings: a -> b in face f's pyramid;
        // the side triangle [b, a, apex] points from that pyramid into the
        // one holding b -> a.
        let mut directed: BTreeMap<(usize, usize), usize> = BTreeMap::new();
        for &f in fs {
            let mut ring = w.faces[f].clone();
            if w.owner[f] != c {
                ring.reverse();
            }
            for i in 0..ring.len() {
                directed.insert((ring[i], ring[(i + 1) % ring.len()]), f);
            }
        }
        for (&(a, b), &f) in &directed {
            if a < b {
                let g = directed[&(b, a)];
                push(vec![b, a, apex], pyr[&(c, f)], Some(pyr[&(c, g)]), None);
            }
        }
    }
    // Internal faces first again (the pipeline's prefix rule).
    let idx: Vec<usize> = (0..faces.len()).filter(|&f| neighbour[f].is_some()).chain((0..faces.len()).filter(|&f| neighbour[f].is_none())).collect();
    w.faces = idx.iter().map(|&f| faces[f].clone()).collect();
    w.owner = idx.iter().map(|&f| owner[f]).collect();
    w.neighbour = idx.iter().map(|&f| neighbour[f]).collect();
    w.region = idx.iter().map(|&f| region[f]).collect();
    w.n_cells = next;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;

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

        let mut w = Work { points: m.points.clone(), faces: m.faces.clone(), owner: m.owner.clone(), neighbour: m.neighbour.clone(), region, n_cells: 8 };
        pyramids(&mut w, &[true; 8]);
        let v = VolumeMesh { points: w.points, faces: w.faces, owner: w.owner, neighbour: w.neighbour, n_cells: w.n_cells, patches: vec![] };
        assert_eq!(v.cell_count(), 48);
        v.validate().unwrap();
        assert!((v.total_volume() - 1.0).abs() < 1e-12);
        assert!(v.owner.iter().zip(&v.neighbour).all(|(&o, n)| n.is_none_or(|n| o < n)));
        assert_eq!(crate::checks::check_quality(&v).n_negative_volume_cells, 0);
    }

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
