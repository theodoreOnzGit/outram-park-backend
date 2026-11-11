// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/boundaryLayers/extrudeLayer/
//     extrudeLayer.{C,H}, extrudeLayerI.H (addressingCalculator)
//   meshLibrary/utilities/meshes/polyMeshGenModifier/
//     polyMeshGenModifierAddCells.C (addCells: faces matched by face==)
//   meshLibrary/utilities/meshes/polyMeshGen/polyMeshGenCells.C
//     (calculateOwnersAndNeighbours: the lower cell owns a face)
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
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

//! cfMesh's **`extrudeLayer`** (gh:#1280): insert a layer of cells at an
//! extrusion front, a list of (face, cell) pairs. Each pair says "make
//! room on this side of this face": the face is copied into the cell, the
//! copy's points are split off and pulled into the cell, and a new cell
//! fills the gap between the copy and the original. It is the engine of
//! `createFundamentalSheets` ([`super::fundamental_sheets`]), which
//! `meshSurfaceEdgeExtractorFUN` runs first.
//!
//! The constructor's five steps, serial:
//!
//! 1. `createDuplicateFrontFaces`: a copy of each front face for each side
//!    named in the front (an internal face named from both sides gets two
//!    copies, and the original is left between the two new cells).
//! 2. `createNewVertices`: at each front point, the faces around it are
//!    grouped by the cells they reach without crossing the front; each
//!    group gets a new point at the same place.
//! 3. `movePoints`: each new point moves against the summed normals of
//!    the copied faces at it, by 0.4 × the smallest centre-to-vertex
//!    distance of those faces (or by the given `thickness`).
//! 4. `createLayerCells`: one cell per copied face (the copy, the original
//!    and a side quad per edge); a hex per edge where two layers
//!    self-intersect, and a corner cell where three or more do.
//! 5. `updateBoundary`: the old boundary faces keep their patches; a new
//!    quad on the boundary takes the patch every one of its new points is
//!    in.
//!
//! **Deviations, all said so:**
//! - **Working form.** Upstream holds cells as face lists and derives owner
//!   and neighbour (the lower cell owns, `calculateOwnersAndNeighbours`);
//!   this port builds the same cell lists from a [`TopoMesh`] (an internal
//!   face with owner > neighbour is reversed first, so the rule holds on
//!   input) and returns a [`TopoMesh`].
//! - **Face normals** are Newell area vectors
//!   ([`crate::surface_engine::ring_area_vector`]), the crate's reading of
//!   cfMesh's `face::normal` (see `surface_engine`); centres are OpenFOAM's
//!   `face::centre` ([`foam_face_centre`]).
//! - **`FatalError`s** are [`ExtrudeLayerError`]s.
//! - **A face left with one cell and no patch** by `updateBoundary`:
//!   upstream's `replaceBoundary` drops it (the cell is then open). It is
//!   kept here with no patch and counted in
//!   [`ExtrudeLayerReport::n_unpatched_boundary_faces`].
//! - **Parallel** (`FRONTVERTEXPROCBND`, `createNewFacesParallel`) is not
//!   applicable.

use crate::correct_edges::foam_face_centre;
use crate::math::Vec3;
use crate::surface_engine::ring_area_vector;
use crate::topology::TopoMesh;
use std::collections::{BTreeMap, BTreeSet};

/// OpenFOAM's `VSMALL` for `double`.
const VSMALL: f64 = 1e-300;
/// OpenFOAM's `VGREAT` for `double`.
const VGREAT: f64 = 1e300;

/// Where upstream's `extrudeLayer` stops with `FatalError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtrudeLayerError {
    /// `createNewVertices`: "Front is not defined at point" (every face at
    /// the point is reachable from every other without crossing the front).
    FrontNotDefinedAtPoint(usize),
    /// `addressingCalculator::faceSharingEdge`: "Expected only one such face",
    /// or (where upstream would index with -1) no such face.
    FaceSharingEdge { extruded: usize, edge: usize },
    /// `createLayerCells`: "Cannot find searched face" / "Cannot find points".
    EdgeCell { extruded: usize, edge: usize },
    /// `createLayerCells`: "Corner cell is invalid".
    CornerCellInvalid(usize),
    /// `updateBoundary`: "Found more than one patch!".
    MoreThanOnePatch(usize),
    /// `calculateOwnersAndNeighbours`: "appears in more than 2 cells".
    FaceInMoreThanTwoCells(usize),
}

impl std::fmt::Display for ExtrudeLayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "extrudeLayer: {self:?}")
    }
}

/// What [`extrude_layer`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtrudeLayerReport {
    /// Copied front faces (`extrudedFaces_`), one layer cell each.
    pub n_extruded_faces: usize,
    /// Points created by `createNewVertices`.
    pub n_new_points: usize,
    /// Hex cells built over self-intersecting edges.
    pub n_edge_cells: usize,
    /// Cells built where three or more layers meet at a point.
    pub n_corner_cells: usize,
    /// New boundary quads given a patch by `updateBoundary`.
    pub n_new_boundary_faces: usize,
    /// Faces with one cell that `updateBoundary` gave no patch (kept here
    /// with no patch; upstream drops them).
    pub n_unpatched_boundary_faces: usize,
}

/// OpenFOAM `face::operator==` (`face::compare != 0`): the same points in
/// the same cyclic order, either direction. Returned as a key.
fn face_key(ring: &[usize]) -> Vec<usize> {
    let n = ring.len();
    let i0 = (0..n).min_by_key(|&i| ring[i]).unwrap_or(0);
    let fwd: Vec<usize> = (0..n).map(|k| ring[(i0 + k) % n]).collect();
    let bwd: Vec<usize> = (0..n).map(|k| ring[(i0 + n - k) % n]).collect();
    fwd.min(bwd)
}

/// `face::reverseFace`: the first point kept, the rest reversed.
fn reverse_face(ring: &[usize]) -> Vec<usize> {
    let n = ring.len();
    (0..n).map(|i| ring[(n - i) % n]).collect()
}

/// `calculateOwnersAndNeighbours`: the lowest cell holding a face owns it.
#[allow(clippy::type_complexity)]
fn owners_and_neighbours(n_faces: usize, cells: &[Vec<usize>]) -> Result<(Vec<Option<usize>>, Vec<Option<usize>>), ExtrudeLayerError> {
    let mut own = vec![None; n_faces];
    let mut nei = vec![None; n_faces];
    for (c, fs) in cells.iter().enumerate() {
        for &f in fs {
            if own[f].is_none() {
                own[f] = Some(c);
            } else if nei[f].is_none() {
                nei[f] = Some(c);
            } else {
                return Err(ExtrudeLayerError::FaceInMoreThanTwoCells(f));
            }
        }
    }
    Ok((own, nei))
}

/// Faces at each point, ascending (`VRWGraph::reverseAddressing`).
fn point_faces(n_points: usize, faces: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut pf: Vec<Vec<usize>> = vec![Vec::new(); n_points];
    for (f, ring) in faces.iter().enumerate() {
        for &p in ring {
            pf[p].push(f);
        }
    }
    pf
}

/// `extrudeLayer::addressingCalculator`: the extruded faces seen through the
/// point-to-extruded-face addressing `pe`.
struct Adc<'a> {
    faces: &'a [Vec<usize>],
    extruded: &'a [(usize, usize)],
    orientation: &'a [bool],
    pe: &'a [Vec<usize>],
}

impl Adc<'_> {
    fn which(ring: &[usize], p: usize) -> usize {
        ring.iter().position(|&q| q == p).unwrap_or(usize::MAX)
    }
    /// `origPointLabel(extrudedI, pos)`.
    fn orig_point_label(&self, e: usize, pos: usize) -> usize {
        let of = &self.faces[self.extruded[e].1];
        if self.orientation[e] {
            of[pos]
        } else {
            of[(of.len() - pos) % of.len()]
        }
    }
    /// `origPoint(extrudedI, pointI)`.
    fn orig_point(&self, e: usize, p: usize) -> usize {
        let pos = Self::which(&self.faces[self.extruded[e].0], p);
        self.orig_point_label(e, pos)
    }
    /// `faceSharingEdge(extrudedI, eI)`: the one other extruded face holding
    /// edge `eI` of face `extrudedI`.
    fn face_sharing_edge(&self, e: usize, ei: usize) -> Result<usize, ExtrudeLayerError> {
        let f = &self.faces[self.extruded[e].0];
        let (p, next) = (f[ei], f[(ei + 1) % f.len()]);
        let mut other = None;
        for &cf in &self.pe[p] {
            if cf == e {
                continue;
            }
            if self.pe[next].contains(&cf) {
                if other.is_some() {
                    return Err(ExtrudeLayerError::FaceSharingEdge { extruded: e, edge: ei });
                }
                other = Some(cf);
            }
        }
        other.ok_or(ExtrudeLayerError::FaceSharingEdge { extruded: e, edge: ei })
    }
    /// `facesSharingEdge(start, end)`.
    fn faces_sharing_edge(&self, start: usize, end: usize) -> Vec<usize> {
        self.pe[start].iter().copied().filter(|cf| self.pe[end].contains(cf)).collect()
    }
}

/// `extrudeLayer(mesh, extrusionFront, thickness)` on `w`. `front` holds
/// (face, cell) pairs of `w`; a pair whose cell is neither the face's owner
/// nor its neighbour is ignored, as upstream. `thickness` `None` is
/// upstream's default −1 (0.4 × the smallest centre-to-vertex distance of
/// the copied faces at each point). Returns the new mesh: points, faces and
/// cells of `w` first (a copied face's original keeps its label), then the
/// new ones.
pub fn extrude_layer(w: &TopoMesh, front: &[(usize, usize)], thickness: Option<f64>) -> Result<(TopoMesh, ExtrudeLayerReport), ExtrudeLayerError> {
    let n_orig_points = w.points.len();
    let n_orig_faces = w.faces.len();
    let mut points = w.points.clone();
    let mut faces = w.faces.clone();
    // The lower cell owns (upstream's rule): reverse any face where it does not.
    let mut owner0 = w.owner.clone();
    let mut neighbour0 = w.neighbour.clone();
    for f in 0..n_orig_faces {
        if let Some(n) = neighbour0[f] {
            if n < owner0[f] {
                faces[f] = reverse_face(&faces[f]);
                neighbour0[f] = Some(owner0[f]);
                owner0[f] = n;
            }
        }
    }
    let mut cells: Vec<Vec<usize>> = w.cell_faces();
    let mut report = ExtrudeLayerReport::default();

    // ---- createDuplicateFrontFaces ----
    let mut face_in_front = vec![usize::MAX; n_orig_faces];
    let mut new_face_labels: Vec<(Option<usize>, Option<usize>)> = Vec::new();
    let mut counter = 0usize;
    for &(f, c) in front {
        if face_in_front[f] == usize::MAX {
            face_in_front[f] = new_face_labels.len();
            new_face_labels.push((None, None));
        }
        let cf = &mut new_face_labels[face_in_front[f]];
        if c == owner0[f] && cf.0.is_none() {
            cf.0 = Some(counter);
            counter += 1;
        } else if neighbour0[f] == Some(c) && cf.1.is_none() {
            cf.1 = Some(counter);
            counter += 1;
        }
    }
    faces.resize(n_orig_faces + counter, Vec::new());
    let mut extruded: Vec<(usize, usize)> = vec![(0, 0); counter];
    let mut orientation = vec![false; counter];
    for f in 0..n_orig_faces {
        if face_in_front[f] == usize::MAX {
            continue;
        }
        let (f_own, f_nei) = new_face_labels[face_in_front[f]];
        if neighbour0[f].is_none() {
            if let Some(o) = f_own {
                faces[n_orig_faces + o] = faces[f].clone();
                extruded[o] = (n_orig_faces + o, f);
                orientation[o] = true;
            }
        } else {
            match (f_own, f_nei) {
                (Some(o), Some(n)) => {
                    if o < n {
                        faces[n_orig_faces + o] = faces[f].clone();
                        extruded[o] = (n_orig_faces + o, f);
                        orientation[o] = true;
                        faces[n_orig_faces + n] = reverse_face(&faces[f]);
                        extruded[n] = (n_orig_faces + n, f);
                        orientation[n] = false;
                    } else {
                        let ring = std::mem::take(&mut faces[f]);
                        faces[f] = reverse_face(&ring);
                        faces[n_orig_faces + o] = ring;
                        extruded[o] = (n_orig_faces + o, f);
                        orientation[o] = false;
                        faces[n_orig_faces + n] = faces[f].clone();
                        extruded[n] = (n_orig_faces + n, f);
                        orientation[n] = true;
                    }
                }
                (Some(o), None) => {
                    let ring = std::mem::take(&mut faces[f]);
                    faces[f] = reverse_face(&ring);
                    faces[n_orig_faces + o] = ring;
                    extruded[o] = (n_orig_faces + o, f);
                    orientation[o] = false;
                }
                (None, Some(n)) => {
                    faces[n_orig_faces + n] = reverse_face(&faces[f]);
                    extruded[n] = (n_orig_faces + n, f);
                    orientation[n] = false;
                }
                (None, None) => {}
            }
        }
    }
    // Renumber the cells.
    for f in 0..n_orig_faces {
        if face_in_front[f] == usize::MAX {
            continue;
        }
        let (a, b) = new_face_labels[face_in_front[f]];
        if let Some(a) = a {
            for x in cells[owner0[f]].iter_mut().filter(|x| **x == f) {
                *x = n_orig_faces + a;
            }
        }
        if let (Some(b), Some(n)) = (b, neighbour0[f]) {
            for x in cells[n].iter_mut().filter(|x| **x == f) {
                *x = n_orig_faces + b;
            }
        }
    }
    report.n_extruded_faces = counter;

    // ---- createNewVertices ----
    let (owner, neighbour) = owners_and_neighbours(faces.len(), &cells)?;
    let mut front_point = vec![false; points.len()];
    for &(e, _) in &extruded {
        for &p in &faces[e] {
            front_point[p] = true;
        }
    }
    let pf = point_faces(points.len(), &faces);
    let mut orig_point_label: Vec<Option<usize>> = vec![None; n_orig_points];
    for p in 0..n_orig_points {
        if !front_point[p] {
            continue;
        }
        let row = &pf[p];
        let mut group_of = vec![usize::MAX; row.len()];
        let mut group = 0usize;
        for i in 0..row.len() {
            if row[i] < n_orig_faces || group_of[i] != usize::MAX {
                continue;
            }
            let mut stack = vec![i];
            group_of[i] = group;
            while let Some(fl) = stack.pop() {
                let (own, nei) = (owner[row[fl]], neighbour[row[fl]]);
                for j in 0..row.len() {
                    if group_of[j] != usize::MAX {
                        continue;
                    }
                    let fj = row[j];
                    let Some(oj) = owner[fj] else { continue };
                    if Some(oj) == own || Some(oj) == nei {
                        group_of[j] = group;
                        stack.push(j);
                    }
                    let Some(nj) = neighbour[fj] else { continue };
                    if Some(nj) == own || Some(nj) == nei {
                        group_of[j] = group;
                        stack.push(j);
                    }
                }
            }
            group += 1;
        }
        if group == 1 && group_of.iter().all(|&g| g == 0) {
            return Err(ExtrudeLayerError::FrontNotDefinedAtPoint(p));
        }
        let first = points.len();
        for _ in 0..group {
            orig_point_label.push(Some(p));
            points.push(points[p]);
        }
        for (i, &f) in row.iter().enumerate() {
            if group_of[i] == usize::MAX {
                continue;
            }
            if let Some(pos) = faces[f].iter().position(|&q| q == p) {
                faces[f][pos] = first + group_of[i];
            }
        }
    }
    report.n_new_points = points.len() - n_orig_points;

    // ---- movePoints ----
    let pf = point_faces(points.len(), &faces);
    let mut displacement = vec![Vec3::ZERO; points.len() - n_orig_points];
    for (k, d) in displacement.iter_mut().enumerate() {
        let p = n_orig_points + k;
        let mut normal = Vec3::ZERO;
        let mut t = VGREAT;
        for &f in &pf[p] {
            if f >= n_orig_faces {
                let ring = &faces[f];
                normal = normal.sub(ring_area_vector(&points, ring));
                if thickness.is_none() {
                    let c = foam_face_centre(&points, ring);
                    let mut dmin = VGREAT;
                    for &q in ring {
                        dmin = dmin.min(points[q].sub(c).length());
                    }
                    t = t.min(dmin);
                }
            }
        }
        let t = thickness.unwrap_or(t * 0.4);
        let m = normal.length();
        if m > VSMALL {
            normal = normal.scale(1.0 / m);
        }
        *d = normal.scale(t);
    }
    for (k, d) in displacement.iter().enumerate() {
        points[n_orig_points + k] = points[n_orig_points + k].add(*d);
    }

    // ---- createLayerCells ----
    // Point -> extruded faces holding it (`pointFaces` over the new faces).
    let mut pe: Vec<Vec<usize>> = vec![Vec::new(); points.len()];
    for (e, &(nf, _)) in extruded.iter().enumerate() {
        for &p in &faces[nf] {
            pe[p].push(e);
        }
    }
    let opl = |p: usize| -> Option<usize> { if p < n_orig_points { None } else { orig_point_label[p] } };
    let mut new_cells: Vec<Vec<Vec<usize>>> = Vec::new();
    for e in 0..extruded.len() {
        let f = &faces[extruded[e].0];
        let of = &faces[extruded[e].1];
        let n = f.len();
        let mut cell = vec![f.clone(), of.clone()];
        for i in 0..n {
            let ef = if orientation[e] {
                vec![f[i], f[(i + 1) % n], of[(i + 1) % n], of[i]]
            } else {
                vec![f[i], f[(i + 1) % n], of[(n - (i + 1) % n) % n], of[(n - i) % n]]
            };
            cell.push(ef);
        }
        new_cells.push(cell);
    }
    let adc = Adc { faces: &faces, extruded: &extruded, orientation: &orientation, pe: &pe };
    // Edge cells (self-intersection over an edge).
    for e in 0..extruded.len() {
        let f = &faces[extruded[e].0];
        for ei in 0..f.len() {
            let p = f[ei];
            let next = f[(ei + 1) % f.len()];
            if pe[p].is_empty() || pe[next].is_empty() || p < next {
                continue;
            }
            let ofp = adc.orig_point(e, p);
            let ofn = adc.orig_point(e, next);
            if opl(p) != opl(ofp) || opl(next) != opl(ofn) {
                continue;
            }
            let other = adc.face_sharing_edge(e, ei)?;
            let oofp = adc.orig_point(other, p);
            let oofn = adc.orig_point(other, next);
            if !(ofp >= n_orig_points && ofn >= n_orig_points) {
                return Err(ExtrudeLayerError::EdgeCell { extruded: e, edge: ei });
            }
            let fc = adc.faces_sharing_edge(ofp, ofn);
            if fc.len() != 1 {
                return Err(ExtrudeLayerError::EdgeCell { extruded: e, edge: ei });
            }
            let op = adc.orig_point(fc[0], ofp);
            let on = adc.orig_point(fc[0], ofn);
            new_cells.push(vec![
                vec![p, ofp, ofn, next],
                vec![p, next, oofn, oofp],
                vec![oofp, oofn, on, op],
                vec![ofp, op, on, ofn],
                vec![p, oofp, op, ofp],
                vec![next, ofn, on, oofn],
            ]);
            report.n_edge_cells += 1;
        }
    }
    // Corner cells (three or more self-intersecting layers at a point).
    for p in 0..points.len() {
        let row = &pe[p];
        if row.len() < 3 {
            continue;
        }
        let ofps: Vec<usize> = row.iter().map(|&e| adc.orig_point(e, p)).collect();
        let distinct = ofps.iter().collect::<BTreeSet<_>>().len() == ofps.len();
        if !distinct {
            continue;
        }
        let k = ofps.len();
        let mut cell: Vec<Vec<usize>> = vec![Vec::new(); 2 * k];
        for (i, &e) in row.iter().enumerate() {
            let f = &faces[extruded[e].0];
            let ei = Adc::which(f, p);
            let ofp = ofps[i];
            let ofn = adc.orig_point_label(e, (ei + 1) % f.len());
            let ff = adc.face_sharing_edge(e, ei)?;
            let pos = row.iter().position(|&x| x == ff).ok_or(ExtrudeLayerError::CornerCellInvalid(p))?;
            let fe = adc.faces_sharing_edge(ofp, ofn);
            let Some(&fe0) = fe.first() else { return Err(ExtrudeLayerError::CornerCellInvalid(p)) };
            let op = adc.orig_point(fe0, ofp);
            cell[i] = vec![p, ofp, op, ofps[pos]];
        }
        for (i, &ofp) in ofps.iter().enumerate() {
            let (mut prev, mut next) = (None, None);
            for j in 0..k {
                if cell[j][1] == ofp {
                    prev = Some(cell[j][2]);
                }
                if cell[j][3] == ofp {
                    next = Some(cell[j][2]);
                }
            }
            let (Some(prev), Some(next), Some(o)) = (prev, next, orig_point_label[p]) else {
                return Err(ExtrudeLayerError::CornerCellInvalid(p));
            };
            cell[k + i] = vec![prev, ofp, next, o];
        }
        new_cells.push(cell);
        report.n_corner_cells += 1;
    }
    // polyMeshGenModifier::addCells: a face equal to an existing one (face==)
    // is that face; the first match is the lowest label.
    let mut by_key: BTreeMap<Vec<usize>, usize> = BTreeMap::new();
    for (f, ring) in faces.iter().enumerate() {
        by_key.entry(face_key(ring)).or_insert(f);
    }
    for cell in new_cells {
        let mut c = Vec::with_capacity(cell.len());
        for ring in cell {
            let key = face_key(&ring);
            let label = match by_key.get(&key) {
                Some(&l) => l,
                None => {
                    faces.push(ring);
                    by_key.insert(key, faces.len() - 1);
                    faces.len() - 1
                }
            };
            c.push(label);
        }
        cells.push(c);
    }

    // ---- updateBoundary ----
    let (owner, neighbour) = owners_and_neighbours(faces.len(), &cells)?;
    let opl_of = |p: usize| -> Option<usize> { if p < n_orig_points { None } else { orig_point_label[p] } };
    // Patches of the old boundary faces at each point (meshSurfacePartitioner::pointPatches).
    let mut point_patches: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); points.len()];
    let mut on_boundary = vec![false; points.len()];
    let mut region: Vec<Option<usize>> = vec![None; faces.len()];
    let unassigned = w.patch_names.len();
    for f in 0..n_orig_faces {
        if w.neighbour[f].is_none() {
            region[f] = w.region[f];
            for &p in &faces[f] {
                on_boundary[p] = true;
                point_patches[p].insert(w.region[f].unwrap_or(unassigned));
            }
        }
    }
    for f in n_orig_faces..faces.len() {
        if owner[f].is_none() || neighbour[f].is_some() {
            continue;
        }
        let ring = &faces[f];
        let mut patch = None;
        if ring.len() == 4 {
            let mut orig_pts: Vec<usize> = Vec::new();
            let mut new_pts: Vec<usize> = Vec::new();
            for &p in ring {
                let o = opl_of(p).unwrap_or(p);
                if !orig_pts.contains(&o) {
                    orig_pts.push(o);
                }
                if opl_of(p).is_some() {
                    new_pts.push(p);
                }
            }
            if orig_pts.len() <= 2 && !new_pts.is_empty() {
                let mut count: BTreeMap<usize, usize> = BTreeMap::new();
                for &p in &new_pts {
                    if !on_boundary[p] {
                        continue;
                    }
                    for &q in &point_patches[p] {
                        *count.entry(q).or_default() += 1;
                    }
                }
                for (&q, &n) in &count {
                    if n == new_pts.len() {
                        if patch.is_some() {
                            return Err(ExtrudeLayerError::MoreThanOnePatch(f));
                        }
                        patch = Some(q);
                    }
                }
            }
        }
        match patch {
            Some(q) => {
                region[f] = if q == unassigned { None } else { Some(q) };
                report.n_new_boundary_faces += 1;
            }
            None => report.n_unpatched_boundary_faces += 1,
        }
    }
    // Assemble, dropping faces no cell holds.
    let keep: Vec<usize> = (0..faces.len()).filter(|&f| owner[f].is_some()).collect();
    let out = TopoMesh {
        points,
        faces: keep.iter().map(|&f| faces[f].clone()).collect(),
        owner: keep.iter().map(|&f| owner[f].unwrap_or(0)).collect(),
        neighbour: keep.iter().map(|&f| neighbour[f]).collect(),
        region: keep.iter().map(|&f| if neighbour[f].is_some() { None } else { region[f] }).collect(),
        n_cells: cells.len(),
        patch_names: w.patch_names.clone(),
        cells: None,
    };
    Ok((out, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;
    use crate::checks::cell_geometry;

    pub(crate) fn box_topo(n: usize) -> TopoMesh {
        TopoMesh::from_volume_mesh(&cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [n, n, n]))
    }

    fn check_valid(w: &TopoMesh) -> (f64, f64) {
        let (m, _) = w.clone().into_volume_mesh();
        m.validate().unwrap();
        let (vols, _) = cell_geometry(&m);
        let vmin = vols.iter().cloned().fold(f64::MAX, f64::min);
        (m.total_volume(), vmin)
    }

    /// V&V (gh:#1280), analytic. **Predictions (written 2026-11-05, before
    /// the first run):** extruding every boundary face of a 3×3×3 unit box
    /// into its owner (`createFundamentalSheetsJFS::createInitialSheet`)
    /// gives 27 + 54 cells, every boundary point gets exactly one new point
    /// (~~98 new~~ **56**: the prediction miscounted; a 4×4×4 point grid has
    /// 64 − 8 = 56 boundary points, and 56 is what the run gave), the
    /// boundary keeps its 54 faces and the volume (exactly 1
    /// to 1e-12, the boundary does not move), every cell positive, no edge
    /// or corner cell (no self-intersection), no new boundary quad.
    #[test]
    fn wrapper_sheet_on_a_box() {
        let w = box_topo(3);
        let front: Vec<(usize, usize)> = (0..w.faces.len()).filter(|&f| w.neighbour[f].is_none()).map(|f| (f, w.owner[f])).collect();
        let (out, r) = extrude_layer(&w, &front, None).unwrap();
        assert_eq!(out.n_cells, 27 + 54);
        assert_eq!(r.n_new_points, 56);
        assert_eq!((r.n_edge_cells, r.n_corner_cells, r.n_new_boundary_faces, r.n_unpatched_boundary_faces), (0, 0, 0, 0));
        assert_eq!(out.neighbour.iter().filter(|n| n.is_none()).count(), 54);
        let (v, vmin) = check_valid(&out);
        assert!((v - 1.0).abs() < 1e-12, "volume {v}");
        assert!(vmin > 0.0);
    }

    /// V&V (gh:#1280), analytic: an internal plane of 9 faces (x = 1/3 of a
    /// 3×3×3 box) named from both sides. **Prediction (written 2026-11-05,
    /// before the first run):** 18 layer cells (two per face, the original
    /// face between them), 16 front points × 2 groups = 32 new points, no
    /// unpatched face, closed, all cells positive. (A guess at the new
    /// boundary-quad count was drafted with it and left unfinished; it is
    /// not asserted.) **Result: held.**
    #[test]
    fn internal_sheet_from_both_sides() {
        let w = box_topo(3);
        let plane: Vec<usize> = (0..w.faces.len())
            .filter(|&f| w.neighbour[f].is_some() && w.faces[f].iter().all(|&p| (w.points[p].x - 1.0 / 3.0).abs() < 1e-12))
            .collect();
        assert_eq!(plane.len(), 9);
        let mut front = Vec::new();
        for &f in &plane {
            front.push((f, w.owner[f]));
            front.push((f, w.neighbour[f].unwrap()));
        }
        let (out, r) = extrude_layer(&w, &front, None).unwrap();
        assert_eq!(r.n_extruded_faces, 18);
        assert_eq!(out.n_cells, 27 + 18);
        assert_eq!(r.n_new_points, 32);
        assert_eq!(r.n_unpatched_boundary_faces, 0);
        let (_, vmin) = check_valid(&out);
        assert!(vmin > 0.0);
    }

    #[test]
    fn face_key_is_face_equality() {
        assert_eq!(face_key(&[3, 1, 2, 4]), face_key(&[1, 2, 4, 3]));
        assert_eq!(face_key(&[3, 1, 2, 4]), face_key(&[4, 2, 1, 3]));
        assert_ne!(face_key(&[3, 1, 2, 4]), face_key(&[3, 2, 1, 4]));
        assert_eq!(reverse_face(&[0, 1, 2, 3]), vec![0, 3, 2, 1]);
    }
}
