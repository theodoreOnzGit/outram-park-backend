// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithm structure derived from OpenFOAM-dev (OpenFOAM Foundation),
// commit 852527377c7e87e33d51b19ca8974b5db60af28b:
//   `src/mesh/blockMesh/blockMesh/blockMeshMerge.C`   (calcMergeInfo)
//   `src/mesh/blockMesh/blockMesh/blockMeshCreate.C`  (createPoints/Cells/Patches)
//   `src/mesh/blockMesh/blocks/block/blockCreate.C`   (cells, createBoundary)
//   `src/OpenFOAM/meshes/polyMesh/polyMeshFromShapeMesh.C` (setTopology)
//   `src/OpenFOAM/meshes/meshShapes/cellMatcher/*`     (degenerateMatcher and
//       the hex/tet/prism/pyr/wedge/tetWedge matchers, cellMatcher base)
//   `src/OpenFOAM/meshes/meshShapes/cellShape/*`       (collapse, collapsedFaces)
//   `etc/cellModels`                                    (face tables)
//   `src/OpenFOAM/meshes/meshShapes/face/face.C`       (compare, sameVertices)
// Copyright (C) 2011-2023 OpenFOAM Foundation
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

//! Upstream's topology machinery for `blockMesh` (gh:#1464): cell models,
//! the degenerate-cell matchers, `polyMesh`-from-shapes face numbering, and
//! the topological point merge.
//!
//! Why this exists: `blockMesh` does not merge points by coordinates. It
//! merges them across the *block faces* that the block topology says are
//! shared, numbers the surviving points by first appearance, and then builds
//! the polyMesh from cell shapes with `polyMesh::setTopology`, which fixes the
//! face, owner and neighbour numbering. Reproducing those algorithms, in the
//! same order, is what lets the output be compared byte for byte
//! with upstream's.
//!
//! Everything here is crate-private: the public surface is
//! [`crate::block_mesh::BlockMeshDict::build`].
//!
//! ## Where this departs from upstream (all deliberate, all stated)
//!
//! - `cellMatcher` aborts (`FatalError`) on an inconsistent cell; here the
//!   matcher returns "no match" and [`CellShape::collapse`] reports an error.
//!   A closed polyhedron never takes that path.
//! - The matchers are called by `degenerateMatcher::match(faces)` only, i.e.
//!   as a single-cell mesh with `owner == 0 == celli`, so every
//!   `owner[...] == celli` test in them is `true`; that is hard-wired here
//!   (`fwd` for `rightHand`, `bwd` for `!rightHand`).
//! - `face::compare` walks `ConstCirculator`s; [`face_compare`] tries each
//!   rotation of the second face, both directions. Identical for faces
//!   without repeated labels (every face of a non-degenerate mesh); for the
//!   topology faces of a degenerate block it may differ in which of several
//!   equivalent rotations it reports, never in whether the faces match.
//! - The `N x N` point search of `calcMergeInfo` between two block faces
//!   uses a spatial hash so that large block faces do not cost `N^2`. It
//!   returns exactly the pairs upstream's loop returns (every pair closer
//!   than `sqrMergeTol`), processed in upstream's order.

use std::collections::HashMap;

use outram_foam_basic_lib::primitives::Vector3;

use crate::MeshError;

/// A face as an ordered loop of point labels.
pub(crate) type Face = Vec<usize>;

// ───────────────────────────────────────────────────────────────────────────
//  face comparison (face::compare, face::sameVertices)
// ───────────────────────────────────────────────────────────────────────────

/// `face::compare`: `1` when `b` is `a` rotated, `-1` when `b` is `a`
/// rotated and reversed, `0` when they are different faces.
pub(crate) fn face_compare(a: &[usize], b: &[usize]) -> i32 {
    let n = a.len();
    if n != b.len() || n == 0 {
        return 0;
    }
    if n == 1 {
        return i32::from(a[0] == b[0]);
    }
    for j in 0..n {
        if b[j] != a[0] {
            continue;
        }
        if (0..n).all(|i| a[i] == b[(j + i) % n]) {
            return 1;
        }
        if (0..n).all(|i| a[i] == b[(j + n - i) % n]) {
            return -1;
        }
    }
    0
}

/// `face::operator==`.
pub(crate) fn faces_equal(a: &[usize], b: &[usize]) -> bool {
    face_compare(a, b) != 0
}

/// `face::sameVertices`: the same multiset of labels (order ignored).
pub(crate) fn same_vertices(a: &[usize], b: &[usize]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().all(|&x| {
        a.iter().filter(|&&y| y == x).count() == b.iter().filter(|&&y| y == x).count()
    })
}

// ───────────────────────────────────────────────────────────────────────────
//  cell models (etc/cellModels)
// ───────────────────────────────────────────────────────────────────────────

/// The cell models `blockMesh` can produce: `hex` for the blocks, and the
/// five shapes a collapsed hex can match (`degenerateMatcher`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CellModel {
    /// 8 points, 6 quads.
    Hex,
    /// 7 points, 2 triangles and 4 quads (`etc/cellModels` `wedge`).
    Wedge,
    /// 6 points, 2 triangles and 3 quads.
    Prism,
    /// 5 points, 1 quad and 4 triangles.
    Pyr,
    /// 4 points, 4 triangles.
    Tet,
    /// 5 points, 2 triangles and 2 quads.
    TetWedge,
}

const HEX_FACES: &[&[usize]] = &[
    &[0, 4, 7, 3],
    &[1, 2, 6, 5],
    &[0, 1, 5, 4],
    &[3, 7, 6, 2],
    &[0, 3, 2, 1],
    &[4, 5, 6, 7],
];
const WEDGE_FACES: &[&[usize]] = &[
    &[0, 2, 1],
    &[3, 4, 5, 6],
    &[0, 3, 6],
    &[1, 2, 5, 4],
    &[0, 1, 4, 3],
    &[0, 6, 5, 2],
];
const PRISM_FACES: &[&[usize]] = &[
    &[0, 2, 1],
    &[3, 4, 5],
    &[0, 3, 5, 2],
    &[1, 2, 5, 4],
    &[0, 1, 4, 3],
];
const PYR_FACES: &[&[usize]] = &[
    &[0, 3, 2, 1],
    &[0, 4, 3],
    &[3, 4, 2],
    &[1, 2, 4],
    &[0, 1, 4],
];
const TET_FACES: &[&[usize]] = &[&[1, 2, 3], &[0, 3, 2], &[0, 1, 3], &[0, 2, 1]];
const TET_WEDGE_FACES: &[&[usize]] = &[&[0, 2, 1], &[0, 1, 3], &[0, 3, 4, 2], &[1, 2, 4, 3]];

impl CellModel {
    /// The model's faces as local point indices (`etc/cellModels`), in the
    /// file's order, wound outward.
    pub(crate) fn faces(self) -> &'static [&'static [usize]] {
        match self {
            CellModel::Hex => HEX_FACES,
            CellModel::Wedge => WEDGE_FACES,
            CellModel::Prism => PRISM_FACES,
            CellModel::Pyr => PYR_FACES,
            CellModel::Tet => TET_FACES,
            CellModel::TetWedge => TET_WEDGE_FACES,
        }
    }

    /// Number of points of the model.
    pub(crate) fn n_points(self) -> usize {
        match self {
            CellModel::Hex => 8,
            CellModel::Wedge => 7,
            CellModel::Prism => 6,
            CellModel::Pyr | CellModel::TetWedge => 5,
            CellModel::Tet => 4,
        }
    }
}

/// A cell: a model and the point labels it is built from (`cellShape`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CellShape {
    /// The model that gives the label order its meaning.
    pub model: CellModel,
    /// Point labels, `model.n_points()` of them.
    pub labels: Vec<usize>,
}

impl CellShape {
    /// `cellShape::faces()`: the model's faces with this shape's labels.
    pub(crate) fn faces(&self) -> Vec<Face> {
        self.model
            .faces()
            .iter()
            .map(|f| f.iter().map(|&i| self.labels[i]).collect())
            .collect()
    }

    /// `cellShape::collapsedFaces()`: the faces with consecutive repeated
    /// labels removed, and faces left with fewer than three points dropped.
    pub(crate) fn collapsed_faces(&self) -> Vec<Face> {
        let mut out = Vec::new();
        for f in self.faces() {
            let mut nf: Face = Vec::with_capacity(f.len());
            let mut prev: Option<usize> = None;
            for &v in &f {
                if prev != Some(v) {
                    nf.push(v);
                    prev = Some(v);
                }
            }
            if nf.len() > 1 && nf[nf.len() - 1] == nf[0] {
                nf.pop();
            }
            if nf.len() > 2 {
                out.push(nf);
            }
        }
        out
    }

    /// `cellShape::collapse()` = `degenerateMatcher::match(collapsedFaces())`.
    ///
    /// Always re-derives the labels, also for a regular hex: the hex matcher
    /// starts from the first face of the collapsed list, so a regular hex
    /// `0..8` comes back as `[0 3 7 4 1 2 6 5]` (checked by a test). That
    /// permutation is part of upstream's face numbering.
    ///
    /// # Errors
    /// [`MeshError::Construction`] when the faces match none of hex, tet,
    /// prism, pyr, wedge, tetWedge (upstream returns the `unknown` model).
    pub(crate) fn collapse(&self) -> Result<CellShape, MeshError> {
        degenerate_match(&self.collapsed_faces()).ok_or_else(|| {
            MeshError::Construction(format!(
                "cell with labels {:?} collapses to none of hex, tet, prism, pyr, wedge, tetWedge",
                self.labels
            ))
        })
    }
}

// ───────────────────────────────────────────────────────────────────────────
//  cellMatcher + degenerateMatcher
// ───────────────────────────────────────────────────────────────────────────

/// `cellMatcher` working state for one cell: `calcLocalFaces`,
/// `calcEdgeAddressing` and `calcPointFaceIndex`.
struct Local {
    /// Faces renumbered by first appearance.
    faces: Vec<Vec<usize>>,
    /// Local to global point labels.
    point_map: Vec<usize>,
    /// `edgeFaces_`: two slots per directed edge key.
    edge_faces: Vec<i64>,
    /// `pointFaceIndex_[vert][face]`: position of `vert` in `face`, or -1.
    point_face_index: Vec<Vec<i64>>,
    n_vert: usize,
}

fn fwd(v: usize, size: usize) -> usize {
    (v + 1) % size
}

fn bwd(v: usize, size: usize) -> usize {
    (size + v - 1) % size
}

impl Local {
    /// `calcLocalFaces` + `calcEdgeAddressing` + `calcPointFaceIndex`.
    /// `None` when the faces do not carry exactly `vert_per_cell` points.
    fn new(faces: &[Face], vert_per_cell: usize) -> Option<Local> {
        let mut local_point: HashMap<usize, usize> = HashMap::new();
        let mut point_map: Vec<usize> = Vec::new();
        let mut local_faces: Vec<Vec<usize>> = Vec::with_capacity(faces.len());
        for f in faces {
            let mut lf = Vec::with_capacity(f.len());
            for &v in f {
                let id = match local_point.get(&v) {
                    Some(&id) => id,
                    None => {
                        if point_map.len() >= vert_per_cell {
                            return None;
                        }
                        let id = point_map.len();
                        local_point.insert(v, id);
                        point_map.push(v);
                        id
                    }
                };
                lf.push(id);
            }
            local_faces.push(lf);
        }
        if point_map.len() != vert_per_cell {
            return None;
        }
        let n = vert_per_cell;
        let mut edge_faces = vec![-1i64; 2 * n * n];
        for (fi, f) in local_faces.iter().enumerate() {
            let mut prev = f.len() - 1;
            for fp in 0..f.len() {
                let (start, end) = (f[prev], f[fp]);
                let key1 = 2 * (start * n + end);
                let key2 = 2 * (end * n + start);
                if edge_faces[key1] == -1 {
                    edge_faces[key1] = fi as i64;
                    edge_faces[key2] = fi as i64;
                } else if edge_faces[key1 + 1] == -1 {
                    edge_faces[key1 + 1] = fi as i64;
                    edge_faces[key2 + 1] = fi as i64;
                } else {
                    return None;
                }
                prev = fp;
            }
        }
        let mut pfi = vec![vec![-1i64; faces.len()]; n];
        for (fi, f) in local_faces.iter().enumerate() {
            for (fp, &v) in f.iter().enumerate() {
                pfi[v][fi] = fp as i64;
            }
        }
        Some(Local {
            faces: local_faces,
            point_map,
            edge_faces,
            point_face_index: pfi,
            n_vert: n,
        })
    }

    /// `cellMatcher::otherFace`: the other face on edge `v0 v1`.
    fn other_face(&self, v0: usize, v1: usize, face: usize) -> Option<usize> {
        let key = 2 * (v0 * self.n_vert + v1);
        if self.edge_faces[key] == face as i64 {
            usize::try_from(self.edge_faces[key + 1]).ok()
        } else if self.edge_faces[key + 1] == face as i64 {
            usize::try_from(self.edge_faces[key]).ok()
        } else {
            None
        }
    }

    /// Position of local point `v` in local face `face`.
    fn pos(&self, v: usize, face: usize) -> Option<usize> {
        usize::try_from(self.point_face_index[v][face]).ok()
    }

    fn size(&self, face: usize) -> usize {
        self.faces[face].len()
    }

    fn label(&self, face: usize, at: usize) -> usize {
        self.point_map[self.faces[face][at]]
    }

    /// First face with `n` points.
    fn first_with(&self, n: usize) -> Option<usize> {
        self.faces.iter().position(|f| f.len() == n)
    }
}

/// Count `(triangles, quads)`; `None` if any face has another size.
fn count_tri_quad(faces: &[Face]) -> Option<(usize, usize)> {
    let (mut t, mut q) = (0, 0);
    for f in faces {
        match f.len() {
            3 => t += 1,
            4 => q += 1,
            _ => return None,
        }
    }
    Some((t, q))
}

/// `hexMatcher::matchShape(checkOnly = false, ...)`.
fn match_hex(faces: &[Face]) -> Option<Vec<usize>> {
    if faces.len() != 6 || faces.iter().any(|f| f.len() != 4) {
        return None;
    }
    let l = Local::new(faces, 8)?;
    let mut vl = vec![0usize; 8];
    // Bottom (face 4) is localFaces_[0]; walk it with `!rightHand` (backwards).
    let f4 = 0usize;
    let n4 = l.size(f4);
    let v0 = 0usize;
    vl[0] = l.label(f4, v0);
    let v1 = bwd(v0, n4);
    vl[1] = l.label(f4, v1);
    let v2 = bwd(v1, n4);
    vl[2] = l.label(f4, v2);
    let v3 = bwd(v2, n4);
    vl[3] = l.label(f4, v3);
    let (p0, p3) = (l.faces[f4][v0], l.faces[f4][v3]);
    let face0 = l.other_face(p3, p0, f4)?;
    let f0v0 = l.pos(p0, face0)?;
    let n0 = l.size(face0);
    let f0v4 = fwd(f0v0, n0);
    vl[4] = l.label(face0, f0v4);
    let f0v7 = fwd(f0v4, n0);
    vl[7] = l.label(face0, f0v7);
    let face5 = l.other_face(l.faces[face0][f0v4], l.faces[face0][f0v7], face0)?;
    let f5v4 = l.pos(l.faces[face0][f0v4], face5)?;
    let n5 = l.size(face5);
    let f5v5 = fwd(f5v4, n5);
    vl[5] = l.label(face5, f5v5);
    let f5v6 = fwd(f5v5, n5);
    vl[6] = l.label(face5, f5v6);
    Some(vl)
}

/// `tetMatcher::matchShape(checkOnly = false, ...)`.
fn match_tet(faces: &[Face]) -> Option<Vec<usize>> {
    if faces.len() != 4 || faces.iter().any(|f| f.len() != 3) {
        return None;
    }
    let l = Local::new(faces, 4)?;
    let mut vl = vec![0usize; 4];
    let f3 = 0usize;
    let n3 = l.size(f3);
    let v0 = 0usize;
    vl[0] = l.label(f3, v0);
    let v1 = bwd(v0, n3);
    vl[1] = l.label(f3, v1);
    let v2 = bwd(v1, n3);
    vl[2] = l.label(f3, v2);
    let face1 = l.other_face(l.faces[f3][v2], l.faces[f3][v0], f3)?;
    let f1v0 = l.pos(l.faces[f3][v0], face1)?;
    let f1v3 = fwd(f1v0, l.size(face1));
    vl[3] = l.label(face1, f1v3);
    Some(vl)
}

/// `pyrMatcher::matchShape(checkOnly = false, ...)`.
fn match_pyr(faces: &[Face]) -> Option<Vec<usize>> {
    if faces.len() != 5 || count_tri_quad(faces)? != (4, 1) {
        return None;
    }
    let l = Local::new(faces, 5)?;
    let mut vl = vec![0usize; 5];
    let f0 = l.first_with(4)?;
    let n0 = l.size(f0);
    let v0 = 0usize;
    vl[0] = l.label(f0, v0);
    let v1 = bwd(v0, n0);
    vl[1] = l.label(f0, v1);
    let v2 = bwd(v1, n0);
    vl[2] = l.label(f0, v2);
    let v3 = bwd(v2, n0);
    vl[3] = l.label(f0, v3);
    let face4 = l.other_face(l.faces[f0][v0], l.faces[f0][v1], f0)?;
    let f4v0 = l.pos(l.faces[f0][v0], face4)?;
    let f4v4 = bwd(f4v0, l.size(face4));
    vl[4] = l.label(face4, f4v4);
    Some(vl)
}

/// `prismMatcher::matchShape(checkOnly = false, ...)`.
fn match_prism(faces: &[Face]) -> Option<Vec<usize>> {
    if faces.len() != 5 || count_tri_quad(faces)? != (2, 3) {
        return None;
    }
    let l = Local::new(faces, 6)?;
    let mut vl = vec![0usize; 6];
    let f0 = l.first_with(3)?;
    let n0 = l.size(f0);
    let v0 = 0usize;
    vl[0] = l.label(f0, v0);
    let v1 = bwd(v0, n0);
    vl[1] = l.label(f0, v1);
    let face4 = l.other_face(l.faces[f0][v0], l.faces[f0][v1], f0)?;
    if l.size(face4) != 4 {
        return None;
    }
    let n4 = l.size(face4);
    let f4v1 = l.pos(l.faces[f0][v1], face4)?;
    let f4v4 = fwd(f4v1, n4);
    vl[4] = l.label(face4, f4v4);
    let f4v3 = fwd(f4v4, n4);
    vl[3] = l.label(face4, f4v3);
    let face1 = l.other_face(l.faces[face4][f4v3], l.faces[face4][f4v4], face4)?;
    if l.size(face1) != 3 {
        return None;
    }
    let v2 = bwd(v1, n0);
    vl[2] = l.label(f0, v2);
    let face3 = l.other_face(l.faces[f0][v1], l.faces[f0][v2], f0)?;
    let f3v2 = l.pos(l.faces[f0][v2], face3)?;
    let f3v5 = fwd(f3v2, l.size(face3));
    vl[5] = l.label(face3, f3v5);
    Some(vl)
}

/// `wedgeMatcher::matchShape(checkOnly = false, ...)`.
fn match_wedge(faces: &[Face]) -> Option<Vec<usize>> {
    if faces.len() != 6 || count_tri_quad(faces)? != (2, 4) {
        return None;
    }
    let l = Local::new(faces, 7)?;
    let f0 = l.first_with(3)?;
    let n0 = l.size(f0);
    for v0 in 0..n0 {
        let mut vl = vec![0usize; 7];
        vl[0] = l.label(f0, v0);
        let v1 = bwd(v0, n0);
        vl[1] = l.label(f0, v1);
        let Some(face4) = l.other_face(l.faces[f0][v0], l.faces[f0][v1], f0) else {
            continue;
        };
        if l.size(face4) != 4 {
            continue;
        }
        let f4v0 = l.pos(l.faces[f0][v0], face4)?;
        let f4v3 = bwd(f4v0, l.size(face4));
        vl[3] = l.label(face4, f4v3);
        let face2 = l.other_face(l.faces[face4][f4v0], l.faces[face4][f4v3], face4)?;
        if l.size(face2) != 3 {
            continue;
        }
        let f2v3 = l.pos(l.faces[face4][f4v3], face2)?;
        let f2v6 = fwd(f2v3, l.size(face2));
        vl[6] = l.label(face2, f2v6);
        let face1 = l.other_face(l.faces[face2][f2v3], l.faces[face2][f2v6], face2)?;
        let f1v6 = l.pos(l.faces[face2][f2v6], face1)?;
        let n1 = l.size(face1);
        let f1v5 = bwd(f1v6, n1);
        vl[5] = l.label(face1, f1v5);
        let f1v4 = bwd(f1v5, n1);
        vl[4] = l.label(face1, f1v4);
        let v2 = bwd(v1, n0);
        vl[2] = l.label(f0, v2);
        return Some(vl);
    }
    None
}

/// `tetWedgeMatcher::matchShape(checkOnly = false, ...)`.
fn match_tet_wedge(faces: &[Face]) -> Option<Vec<usize>> {
    if faces.len() != 4 || count_tri_quad(faces)? != (2, 2) {
        return None;
    }
    let l = Local::new(faces, 5)?;
    let f0 = l.first_with(3)?;
    let n0 = l.size(f0);
    for v0 in 0..n0 {
        let mut vl = vec![0usize; 5];
        vl[0] = l.label(f0, v0);
        let v1 = bwd(v0, n0);
        vl[1] = l.label(f0, v1);
        let face1 = l.other_face(l.faces[f0][v0], l.faces[f0][v1], f0)?;
        if l.size(face1) != 3 {
            continue;
        }
        let v2 = bwd(v1, n0);
        vl[2] = l.label(f0, v2);
        let face3 = l.other_face(l.faces[f0][v1], l.faces[f0][v2], f0)?;
        let f3v2 = l.pos(l.faces[f0][v2], face3)?;
        let n3 = l.size(face3);
        let f3v4 = fwd(f3v2, n3);
        vl[4] = l.label(face3, f3v4);
        let f3v3 = fwd(f3v4, n3);
        vl[3] = l.label(face3, f3v3);
        return Some(vl);
    }
    None
}

/// `degenerateMatcher::match(faces)`, recognising in upstream's order: hex,
/// tet, prism, pyr, wedge, tetWedge.
pub(crate) fn degenerate_match(faces: &[Face]) -> Option<CellShape> {
    let shape = |model, labels| Some(CellShape { model, labels });
    if let Some(l) = match_hex(faces) {
        shape(CellModel::Hex, l)
    } else if let Some(l) = match_tet(faces) {
        shape(CellModel::Tet, l)
    } else if let Some(l) = match_prism(faces) {
        shape(CellModel::Prism, l)
    } else if let Some(l) = match_pyr(faces) {
        shape(CellModel::Pyr, l)
    } else if let Some(l) = match_wedge(faces) {
        shape(CellModel::Wedge, l)
    } else if let Some(l) = match_tet_wedge(faces) {
        shape(CellModel::TetWedge, l)
    } else {
        None
    }
}

// ───────────────────────────────────────────────────────────────────────────
//  polyMesh from cell shapes (polyMesh::setTopology)
// ───────────────────────────────────────────────────────────────────────────

/// The face addressing `polyMesh` builds from cell shapes.
#[derive(Debug, Clone)]
pub(crate) struct ShapeMesh {
    /// Faces: internal, then patch faces patch by patch, then default faces.
    /// Each is the owning cell's own face (outward from the owner).
    pub faces: Vec<Face>,
    /// `cells[c]` = face labels of cell `c`, in the cell model's face order.
    pub cells: Vec<Vec<usize>>,
    /// Owner cell of each face.
    pub owner: Vec<usize>,
    /// Neighbour cell of each internal face.
    pub neighbour: Vec<usize>,
    /// Start face of each given patch.
    pub patch_starts: Vec<usize>,
    /// Size of each given patch.
    pub patch_sizes: Vec<usize>,
    /// First face of the default patch (`faces.len()` if it is empty).
    pub default_start: usize,
}

impl ShapeMesh {
    /// Number of internal faces.
    pub(crate) fn n_internal(&self) -> usize {
        self.neighbour.len()
    }
}

/// `polyMesh::polyMesh(..., cellsAsShapes, boundaryFaces, ...)` addressing:
/// `cellShapePointCells`, `setTopology`, `initMesh(cells)`.
///
/// `boundary_faces[p]` are the faces of patch `p` in any winding; each is
/// matched to a cell face by [`same_vertices`]. Faces no patch names go to a
/// trailing default patch (`default_start..`).
///
/// # Errors
/// [`MeshError::Construction`] when a patch face belongs to no cell, is
/// already an internal face or belongs to another patch (upstream's
/// `FatalError`s).
pub(crate) fn mesh_from_shapes(
    n_points: usize,
    shapes: &[CellShape],
    boundary_faces: &[Vec<Face>],
    patch_names: &[String],
) -> Result<ShapeMesh, MeshError> {
    let n_cells = shapes.len();
    let cell_faces: Vec<Vec<Face>> = shapes.iter().map(CellShape::faces).collect();
    let mut cells: Vec<Vec<i64>> = cell_faces.iter().map(|f| vec![-1i64; f.len()]).collect();
    let mut faces: Vec<Face> = Vec::new();

    // cellShapePointCells
    let mut point_cells: Vec<Vec<usize>> = vec![Vec::new(); n_points];
    for (i, s) in shapes.iter().enumerate() {
        for &p in &s.labels {
            point_cells[p].push(i);
        }
    }

    for celli in 0..n_cells {
        let cur_faces = &cell_faces[celli];
        let mut nei_cells: Vec<i64> = vec![-1; cur_faces.len()];
        let mut face_of_nei: Vec<i64> = vec![-1; cur_faces.len()];
        let mut n_neighbours = 0usize;
        for (facei, cur_face) in cur_faces.iter().enumerate() {
            if cells[celli][facei] >= 0 {
                continue;
            }
            let mut found = false;
            'points: for &pt in cur_face {
                for &cur_nei in &point_cells[pt] {
                    if cur_nei > celli {
                        for (nei_facei, sf) in cell_faces[cur_nei].iter().enumerate() {
                            if faces_equal(sf, cur_face) {
                                found = true;
                                nei_cells[facei] = cur_nei as i64;
                                face_of_nei[facei] = nei_facei as i64;
                                n_neighbours += 1;
                                break;
                            }
                        }
                        if found {
                            break 'points;
                        }
                    }
                }
            }
        }
        // Add the faces in increasing order of neighbour.
        for _ in 0..n_neighbours {
            let mut next: Option<usize> = None;
            let mut min_nei = n_cells as i64;
            for (nc_i, &nc) in nei_cells.iter().enumerate() {
                if nc > -1 && nc < min_nei {
                    next = Some(nc_i);
                    min_nei = nc;
                }
            }
            let Some(next) = next else {
                return Err(MeshError::Construction(
                    "error in internal face insertion".into(),
                ));
            };
            let label = faces.len() as i64;
            faces.push(cur_faces[next].clone());
            cells[celli][next] = label;
            cells[nei_cells[next] as usize][face_of_nei[next] as usize] = label;
            nei_cells[next] = -1;
        }
    }
    let n_internal = faces.len();

    // Boundary faces, patch by patch.
    let mut patch_starts = Vec::with_capacity(boundary_faces.len());
    let mut patch_sizes = Vec::with_capacity(boundary_faces.len());
    for (patchi, patch_faces) in boundary_faces.iter().enumerate() {
        let start = faces.len();
        for (facei, cur_face) in patch_faces.iter().enumerate() {
            // facePatchFaceCells
            let mut cell_inside: Option<usize> = None;
            'search: for &pt in cur_face {
                for &c in &point_cells[pt] {
                    if cell_faces[c].iter().any(|cf| same_vertices(cf, cur_face)) {
                        cell_inside = Some(c);
                        break 'search;
                    }
                }
            }
            let Some(cell_inside) = cell_inside else {
                return Err(MeshError::Construction(format!(
                    "face {facei} in patch {patchi} ({}) does not have a neighbour cell: {cur_face:?}",
                    patch_names.get(patchi).map_or("", String::as_str)
                )));
            };
            let Some(cf) = cell_faces[cell_inside]
                .iter()
                .position(|cf| same_vertices(cf, cur_face))
            else {
                return Err(MeshError::Construction(format!(
                    "face {facei} of patch {patchi} does not seem to belong to cell {cell_inside}"
                )));
            };
            if cells[cell_inside][cf] >= 0 {
                return Err(MeshError::Construction(format!(
                    "trying to specify a boundary face {cur_face:?} on the face on cell \
                     {cell_inside} which is either an internal face or already belongs to some \
                     other patch (face {facei} of patch {patchi} named {})",
                    patch_names.get(patchi).map_or("", String::as_str)
                )));
            }
            cells[cell_inside][cf] = faces.len() as i64;
            faces.push(cell_faces[cell_inside][cf].clone());
        }
        patch_starts.push(start);
        patch_sizes.push(faces.len() - start);
    }

    // Default patch: the faces nobody named, in cell order.
    let default_start = faces.len();
    for (celli, cf) in cells.iter_mut().enumerate() {
        for (facei, slot) in cf.iter_mut().enumerate() {
            if *slot == -1 {
                *slot = faces.len() as i64;
                faces.push(cell_faces[celli][facei].clone());
            }
        }
    }

    // initMesh(cells): owner on first visit, neighbour on the second.
    let mut owner = vec![usize::MAX; faces.len()];
    let mut neighbour_of = vec![usize::MAX; faces.len()];
    let mut n_int = 0usize;
    for (celli, cf) in cells.iter().enumerate() {
        for &f in cf {
            let f = f as usize;
            if owner[f] == usize::MAX {
                owner[f] = celli;
            } else {
                neighbour_of[f] = celli;
                n_int += 1;
            }
        }
    }
    if n_int != n_internal {
        return Err(MeshError::Construction(format!(
            "{n_int} faces are shared by two cells, but {n_internal} internal faces were inserted"
        )));
    }
    let neighbour = neighbour_of[..n_internal].to_vec();
    let cells = cells
        .into_iter()
        .map(|c| c.into_iter().map(|f| f as usize).collect())
        .collect();
    Ok(ShapeMesh {
        faces,
        cells,
        owner,
        neighbour,
        patch_starts,
        patch_sizes,
        default_start,
    })
}

// ───────────────────────────────────────────────────────────────────────────
//  the blocks
// ───────────────────────────────────────────────────────────────────────────

/// One block's generated points (raw, unscaled) and its cell/boundary
/// addressing: `block::createPoints` output plus `block::cells()` and
/// `block::createBoundary()`.
#[derive(Debug, Clone)]
pub(crate) struct BlockGrid {
    /// Cells per direction `(ni, nj, nk)`.
    pub n: [usize; 3],
    /// Points, `pointLabel(i, j, k) = i + j (ni+1) + k (ni+1)(nj+1)`.
    pub points: Vec<Vector3>,
}

impl BlockGrid {
    /// `blockDescriptor::nPoints`.
    pub(crate) fn n_points(&self) -> usize {
        (self.n[0] + 1) * (self.n[1] + 1) * (self.n[2] + 1)
    }

    /// `blockDescriptor::nCells`.
    pub(crate) fn n_cells(&self) -> usize {
        self.n[0] * self.n[1] * self.n[2]
    }

    /// `blockDescriptor::pointLabel`.
    pub(crate) fn point_label(&self, i: usize, j: usize, k: usize) -> usize {
        i + j * (self.n[0] + 1) + k * (self.n[0] + 1) * (self.n[1] + 1)
    }

    /// `block::cells()`: the 8 block-local point labels of each cell, `i`
    /// fastest.
    pub(crate) fn cells(&self) -> Vec<[usize; 8]> {
        let [ni, nj, nk] = self.n;
        let mut out = Vec::with_capacity(self.n_cells());
        for k in 0..nk {
            for j in 0..nj {
                for i in 0..ni {
                    out.push([
                        self.point_label(i, j, k),
                        self.point_label(i + 1, j, k),
                        self.point_label(i + 1, j + 1, k),
                        self.point_label(i, j + 1, k),
                        self.point_label(i, j, k + 1),
                        self.point_label(i + 1, j, k + 1),
                        self.point_label(i + 1, j + 1, k + 1),
                        self.point_label(i, j + 1, k + 1),
                    ]);
                }
            }
        }
        out
    }

    /// `block::createBoundary()`: the quads on each of the six block faces
    /// (x-min, x-max, y-min, y-max, z-min, z-max), in upstream's loop order.
    pub(crate) fn boundary_patches(&self) -> [Vec<[usize; 4]>; 6] {
        let [ni, nj, nk] = self.n;
        let p = |i, j, k| self.point_label(i, j, k);
        let mut b: [Vec<[usize; 4]>; 6] = Default::default();
        for k in 0..nk {
            for j in 0..nj {
                b[0].push([p(0, j, k), p(0, j, k + 1), p(0, j + 1, k + 1), p(0, j + 1, k)]);
            }
        }
        for k in 0..nk {
            for j in 0..nj {
                b[1].push([p(ni, j, k), p(ni, j + 1, k), p(ni, j + 1, k + 1), p(ni, j, k + 1)]);
            }
        }
        for i in 0..ni {
            for k in 0..nk {
                b[2].push([p(i, 0, k), p(i + 1, 0, k), p(i + 1, 0, k + 1), p(i, 0, k + 1)]);
            }
        }
        for i in 0..ni {
            for k in 0..nk {
                b[3].push([p(i, nj, k), p(i, nj, k + 1), p(i + 1, nj, k + 1), p(i + 1, nj, k)]);
            }
        }
        for i in 0..ni {
            for j in 0..nj {
                b[4].push([p(i, j, 0), p(i, j + 1, 0), p(i + 1, j + 1, 0), p(i + 1, j, 0)]);
            }
        }
        for i in 0..ni {
            for j in 0..nj {
                b[5].push([p(i, j, nk), p(i + 1, j, nk), p(i + 1, j + 1, nk), p(i, j + 1, nk)]);
            }
        }
        b
    }
}

// ───────────────────────────────────────────────────────────────────────────
//  blockMesh::calcMergeInfo
// ───────────────────────────────────────────────────────────────────────────

/// OpenFOAM's `small` for double precision.
const SMALL: f64 = 1.0e-15;
/// OpenFOAM's `great` for double precision.
const GREAT: f64 = 1.0e15;

/// Result of `blockMesh::calcMergeInfo`.
#[derive(Debug, Clone)]
pub(crate) struct MergeInfo {
    /// First global (unmerged) point label of each block.
    pub block_offsets: Vec<usize>,
    /// Unmerged global point label to merged point label.
    pub merge_list: Vec<usize>,
    /// Number of merged points.
    pub n_points: usize,
    /// Total cells.
    pub n_cells: usize,
}

fn mag_sqr(v: Vector3) -> f64 {
    v.x * v.x + v.y * v.y + v.z * v.z
}

/// `blockMesh::calcMergeInfo()` (the non-`fastMerge` path).
///
/// `topo` is the block topology (blocks as hex cells over `topo_points`, the
/// raw vertices); `blocks[b]` the points and addressing of block `b`.
///
/// # Errors
/// [`MeshError::Construction`] for the inconsistent-grading and
/// unmergeable-point cases upstream raises (`Inconsistent point locations
/// between block pair`, `Unable to merge point`, ...).
pub(crate) fn calc_merge_info(
    blocks: &[BlockGrid],
    topo: &ShapeMesh,
    topo_points: &[Vector3],
    check_face_correspondence: bool,
) -> Result<MergeInfo, MeshError> {
    let mut block_offsets = Vec::with_capacity(blocks.len());
    let mut n_points = 0usize;
    let mut n_cells = 0usize;
    for b in blocks {
        block_offsets.push(n_points);
        n_points += b.n_points();
        n_cells += b.n_cells();
    }
    let boundary: Vec<[Vec<[usize; 4]>; 6]> = blocks.iter().map(BlockGrid::boundary_patches).collect();

    let mut merge_list: Vec<i64> = vec![-1; n_points];
    let n_topo_faces = topo.faces.len();
    let n_int_topo = topo.n_internal();
    let mut glue: Vec<Vec<[usize; 4]>> = vec![Vec::new(); n_int_topo];

    let set_pair = |ml: &mut Vec<i64>, a: usize, b: usize| {
        let mut m = a.min(b) as i64;
        if ml[a] != -1 {
            m = m.min(ml[a]);
        }
        if ml[b] != -1 {
            m = m.min(ml[b]);
        }
        ml[a] = m;
        ml[b] = m;
    };

    for bf in 0..n_topo_faces {
        let bp = topo.owner[bf];
        let bp_points = &blocks[bp].points;
        let Some(bp_face_label) = topo.cells[bp].iter().position(|&f| f == bf) else {
            return Err(MeshError::Construction(format!(
                "cannot find merge face for block {bp}"
            )));
        };
        let bp_faces = &boundary[bp][bp_face_label];

        // Bounding box of the block's topology points (cell::points).
        let (mut lo, mut hi) = (
            Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
            Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
        );
        for &fl in &topo.cells[bp] {
            for &pl in &topo.faces[fl] {
                let p = topo_points[pl];
                lo = Vector3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
                hi = Vector3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
            }
        }
        let span = hi - lo;
        let merge_sqr_dist = mag_sqr(span * (50.0 * SMALL));

        let mut sqr_merge_tol = GREAT;
        for quad in bp_faces {
            for a in 0..4 {
                for b in 0..4 {
                    if a == b {
                        continue;
                    }
                    let d = mag_sqr(bp_points[quad[a]] - bp_points[quad[b]]);
                    if d < merge_sqr_dist {
                        set_pair(
                            &mut merge_list,
                            quad[a] + block_offsets[bp],
                            quad[b] + block_offsets[bp],
                        );
                    } else {
                        sqr_merge_tol = sqr_merge_tol.min(d);
                    }
                }
            }
        }
        sqr_merge_tol /= 10.0;

        if bf < n_int_topo {
            let bn = topo.neighbour[bf];
            let bn_points = &blocks[bn].points;
            let Some(bn_face_label) = topo.cells[bn]
                .iter()
                .position(|&f| faces_equal(&topo.faces[f], &topo.faces[bf]))
            else {
                return Err(MeshError::Construction(format!(
                    "cannot find merge face for block {bn}"
                )));
            };
            let bn_faces = &boundary[bn][bn_face_label];
            if check_face_correspondence && bp_faces.len() != bn_faces.len() {
                return Err(MeshError::Construction(format!(
                    "inconsistent number of faces between block pair {bp} and {bn}"
                )));
            }

            // All (face, corner) entries of the neighbour face, in upstream's
            // loop order, hashed for the pair search.
            let h = sqr_merge_tol.sqrt();
            let use_grid = h.is_finite() && h > 0.0;
            let cell_of = |p: Vector3| -> (i64, i64, i64) {
                (
                    (p.x / h).floor() as i64,
                    (p.y / h).floor() as i64,
                    (p.z / h).floor() as i64,
                )
            };
            let mut n_entries: Vec<usize> = Vec::with_capacity(bn_faces.len() * 4);
            for quad in bn_faces {
                n_entries.extend_from_slice(quad);
            }
            let mut grid: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
            if use_grid {
                for (e, &np) in n_entries.iter().enumerate() {
                    grid.entry(cell_of(bn_points[np])).or_default().push(e);
                }
            }

            let mut face_pairs: Vec<[usize; 4]> = Vec::with_capacity(bp_faces.len());
            for quad in bp_faces {
                let mut cp = [usize::MAX; 4];
                for (a, &pp) in quad.iter().enumerate() {
                    let pos = bp_points[pp];
                    let mut hits: Vec<usize> = Vec::new();
                    if use_grid {
                        let (cx, cy, cz) = cell_of(pos);
                        for dx in -1..=1 {
                            for dy in -1..=1 {
                                for dz in -1..=1 {
                                    if let Some(v) = grid.get(&(cx + dx, cy + dy, cz + dz)) {
                                        hits.extend(v.iter().copied().filter(|&e| {
                                            mag_sqr(pos - bn_points[n_entries[e]]) < sqr_merge_tol
                                        }));
                                    }
                                }
                            }
                        }
                        hits.sort_unstable();
                    } else {
                        for (e, &np) in n_entries.iter().enumerate() {
                            if mag_sqr(pos - bn_points[np]) < sqr_merge_tol {
                                hits.push(e);
                            }
                        }
                    }
                    for e in hits {
                        let np = n_entries[e];
                        cp[a] = np;
                        set_pair(
                            &mut merge_list,
                            pp + block_offsets[bp],
                            np + block_offsets[bn],
                        );
                    }
                }
                if cp.iter().any(|&c| c == usize::MAX) {
                    return Err(MeshError::Construction(format!(
                        "inconsistent point locations between block pair {bp} and {bn}: \
                         probably due to inconsistent grading"
                    )));
                }
                face_pairs.push(cp);
            }
            glue[bf] = face_pairs;
        }
    }

    // Propagate until stable.
    let mut n_passes = 0usize;
    loop {
        let mut changed = false;
        n_passes += 1;
        for bf in 0..n_int_topo {
            let bp = topo.owner[bf];
            let bn = topo.neighbour[bf];
            let bp_face_label = topo.cells[bp]
                .iter()
                .position(|&f| faces_equal(&topo.faces[f], &topo.faces[bf]))
                .unwrap_or(topo.cells[bp].len());
            let Some(bp_faces) = boundary[bp].get(bp_face_label) else {
                return Err(MeshError::Construction(format!(
                    "cannot find merge face for block {bp}"
                )));
            };
            for (fi, quad) in bp_faces.iter().enumerate() {
                let cp = glue[bf][fi];
                for a in 0..4 {
                    let pl = quad[a] + block_offsets[bp];
                    let nl = cp[a] + block_offsets[bn];
                    if merge_list[pl] != merge_list[nl] {
                        changed = true;
                        let m = merge_list[pl].min(merge_list[nl]);
                        merge_list[pl] = m;
                        merge_list[nl] = m;
                    }
                }
            }
        }
        if n_passes > 100 {
            return Err(MeshError::Construction(
                "point merging failed after max number of passes".into(),
            ));
        }
        if !changed {
            break;
        }
    }

    // Every point on an internal block face must have been merged.
    for bf in 0..n_int_topo {
        let bp = topo.owner[bf];
        let bn = topo.neighbour[bf];
        for (blk, side) in [(bp, "owner"), (bn, "neighbour")] {
            let Some(fl) = topo.cells[blk]
                .iter()
                .position(|&f| faces_equal(&topo.faces[f], &topo.faces[bf]))
            else {
                return Err(MeshError::Construction(format!(
                    "cannot find merge face for block {blk}"
                )));
            };
            for quad in &boundary[blk][fl] {
                for &p in quad {
                    if merge_list[p + block_offsets[blk]] == -1 {
                        return Err(MeshError::Construction(format!(
                            "unable to merge point {p} {:?} of face {fl} of {side} block {blk}",
                            blocks[blk].points[p]
                        )));
                    }
                }
            }
        }
    }

    // Number the unique points.
    let mut n_uniq = 0usize;
    let mut out: Vec<usize> = vec![0; n_points];
    for pointi in 0..n_points {
        let m = merge_list[pointi];
        if m > pointi as i64 {
            return Err(MeshError::Construction(
                "merge list contains point index out of range".into(),
            ));
        }
        if m == -1 || m == pointi as i64 {
            out[pointi] = n_uniq;
            n_uniq += 1;
            merge_list[pointi] = out[pointi] as i64;
        } else {
            // upstream: mergeList_[pointi] = mergeList_[mergeList_[pointi]]
            // with the earlier entries already renumbered.
            let target = merge_list[m as usize];
            out[pointi] = target as usize;
            merge_list[pointi] = target;
        }
    }

    Ok(MergeInfo {
        block_offsets,
        merge_list: out,
        n_points: n_uniq,
        n_cells,
    })
}

// ───────────────────────────────────────────────────────────────────────────
//  blockMesh::createPoints / createCells / createPatches
// ───────────────────────────────────────────────────────────────────────────

/// `blockMesh::createPoints()`: merged points, `scale * block point`. A merged
/// point takes the value written by the **last** block that holds it, as in
/// upstream's loop.
pub(crate) fn create_points(blocks: &[BlockGrid], merge: &MergeInfo, scale: f64) -> Vec<Vector3> {
    let mut points = vec![Vector3::ZERO; merge.n_points];
    for (bi, b) in blocks.iter().enumerate() {
        for (pi, &p) in b.points.iter().enumerate() {
            points[merge.merge_list[merge.block_offsets[bi] + pi]] = p * scale;
        }
    }
    points
}

/// `blockMesh::createCells()`: every block cell as a hex over merged labels,
/// collapsed by `cellShape(hex, labels, true)`.
///
/// # Errors
/// [`MeshError::Construction`] when a cell collapses to no known shape.
pub(crate) fn create_cells(
    blocks: &[BlockGrid],
    merge: &MergeInfo,
) -> Result<Vec<CellShape>, MeshError> {
    let mut cells = Vec::with_capacity(merge.n_cells);
    for (bi, b) in blocks.iter().enumerate() {
        for c in b.cells() {
            let labels: Vec<usize> = c
                .iter()
                .map(|&l| merge.merge_list[l + merge.block_offsets[bi]])
                .collect();
            let hex = CellShape {
                model: CellModel::Hex,
                labels,
            };
            cells.push(hex.collapse()?);
        }
    }
    Ok(cells)
}

/// `blockMesh::createPatchFaces()` for one topology patch.
///
/// `patch_faces` are the topology patch's faces (block faces) and
/// `face_cells` their owner blocks; `block_shapes[b]` is block `b`'s hex.
/// Faces that collapse to an edge or a point are dropped; one that collapses
/// to three points becomes a triangle.
pub(crate) fn create_patch_faces(
    blocks: &[BlockGrid],
    block_shapes: &[CellShape],
    merge: &MergeInfo,
    patch_faces: &[Face],
    face_cells: &[usize],
) -> Vec<Face> {
    let mut out: Vec<Face> = Vec::new();
    for (pf, &blocki) in patch_faces.iter().zip(face_cells) {
        let block_faces = block_shapes[blocki].faces();
        let boundary = blocks[blocki].boundary_patches();
        for (bfl, bface) in block_faces.iter().enumerate() {
            if !faces_equal(bface, pf) {
                continue;
            }
            for quad in &boundary[bfl] {
                let ml = |q: usize| merge.merge_list[quad[q] + merge.block_offsets[blocki]];
                let mut qf = [ml(0), 0, 0, 0];
                let mut n_unique = 1usize;
                for fp in 1..4 {
                    qf[n_unique] = ml(fp);
                    if qf[n_unique] != qf[n_unique - 1] {
                        n_unique += 1;
                    }
                }
                if qf[n_unique - 1] == qf[0] {
                    n_unique -= 1;
                }
                if n_unique == 4 || n_unique == 3 {
                    out.push(qf[..n_unique].to_vec());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_identity() -> CellShape {
        CellShape {
            model: CellModel::Hex,
            labels: (0..8).collect(),
        }
    }

    #[test]
    fn face_compare_rotation_and_reversal() {
        assert_eq!(face_compare(&[0, 1, 2, 3], &[2, 3, 0, 1]), 1);
        assert_eq!(face_compare(&[0, 1, 2, 3], &[3, 2, 1, 0]), -1);
        assert_eq!(face_compare(&[0, 1, 2, 3], &[0, 1, 3, 2]), 0);
        assert_eq!(face_compare(&[0, 1, 2], &[0, 1, 2, 3]), 0);
        assert_eq!(face_compare(&[5], &[5]), 1);
        assert!(faces_equal(&[0, 1, 2], &[1, 2, 0]));
        assert!(same_vertices(&[0, 1, 2, 3], &[3, 1, 0, 2]));
        assert!(!same_vertices(&[0, 1, 2, 3], &[3, 1, 0, 4]));
    }

    #[test]
    fn regular_hex_collapse_is_upstreams_rotation() {
        // Derived by hand from hexMatcher::matchShape (see the module docs):
        // the first face [0 4 7 3] becomes the bottom, walked backwards.
        let c = hex_identity().collapse().unwrap();
        assert_eq!(c.model, CellModel::Hex);
        assert_eq!(c.labels, vec![0, 3, 7, 4, 1, 2, 6, 5]);
        // The rotated hex has the same set of faces.
        let mut a: Vec<Vec<usize>> = hex_identity().faces();
        let mut b: Vec<Vec<usize>> = c.faces();
        for f in a.iter().chain(b.iter()) {
            assert_eq!(f.len(), 4);
        }
        a.sort_by_key(|f| {
            let mut s = f.clone();
            s.sort_unstable();
            s
        });
        b.sort_by_key(|f| {
            let mut s = f.clone();
            s.sort_unstable();
            s
        });
        for (fa, fb) in a.iter().zip(&b) {
            assert!(faces_equal(fa, fb), "{fa:?} vs {fb:?}");
        }
    }

    #[test]
    fn collapsed_hex_becomes_prism_wedge_pyr_tet() {
        let model = |labels: [usize; 8]| {
            CellShape {
                model: CellModel::Hex,
                labels: labels.to_vec(),
            }
            .collapse()
            .unwrap()
            .model
        };
        // Edge 0-3 collapsed: a prism.
        assert_eq!(model([0, 1, 2, 0, 4, 5, 6, 4]), CellModel::Prism);
        // A whole face (4 5 6 7 -> one point): a pyramid.
        assert_eq!(model([0, 1, 2, 3, 4, 4, 4, 4]), CellModel::Pyr);
        // Two opposite edges (0-3 and 4-7) collapsed: a prism too, 6 points.
        assert_eq!(model([0, 1, 2, 0, 4, 5, 6, 4]), CellModel::Prism);
        // Collapsing 3 vertices of a face to one: wedge-like 2 tri + 4 quad
        // is the axisymmetric case: edges 0-3 and 1-2 collapsed? that is a
        // prism with the triangles on x-faces.
        assert_eq!(model([0, 1, 1, 0, 4, 5, 5, 4]), CellModel::Prism);
        // 4 distinct points of a hex that form a tet.
        assert_eq!(model([0, 1, 1, 0, 3, 3, 3, 3]), CellModel::Tet);
    }

    fn two_hex_labels() -> Vec<CellShape> {
        // Two unit cubes sharing the x=1 face: points 0..12 as a 3x2x2 grid.
        let p = |i: usize, j: usize, k: usize| i + 3 * (j + 2 * k);
        let hex = |i0: usize| CellShape {
            model: CellModel::Hex,
            labels: vec![
                p(i0, 0, 0),
                p(i0 + 1, 0, 0),
                p(i0 + 1, 1, 0),
                p(i0, 1, 0),
                p(i0, 0, 1),
                p(i0 + 1, 0, 1),
                p(i0 + 1, 1, 1),
                p(i0, 1, 1),
            ],
        };
        vec![hex(0), hex(1)]
    }

    #[test]
    fn mesh_from_shapes_two_cells_and_default_patch() {
        let shapes = two_hex_labels();
        let m = mesh_from_shapes(12, &shapes, &[], &[]).unwrap();
        assert_eq!(m.n_internal(), 1);
        assert_eq!(m.faces.len(), 11);
        assert_eq!(m.default_start, 1);
        // The internal face is cell 0's x-max face, owned by 0.
        assert_eq!(m.owner[0], 0);
        assert_eq!(m.neighbour[0], 1);
        assert!(faces_equal(&m.faces[0], &[1, 7, 10, 4]));
        // With the x-min face of cell 0 named as a patch it comes first.
        let patch = vec![vec![vec![0usize, 6, 9, 3]]];
        let m = mesh_from_shapes(12, &shapes, &patch, &["left".to_string()]).unwrap();
        assert_eq!(m.patch_starts, vec![1]);
        assert_eq!(m.patch_sizes, vec![1]);
        assert_eq!(m.default_start, 2);
        assert!(same_vertices(&m.faces[1], &[0, 6, 9, 3]));
        // The same patch face twice is an error (already belongs to a patch).
        let dup = vec![vec![vec![0usize, 6, 9, 3], vec![0usize, 6, 9, 3]]];
        assert!(mesh_from_shapes(12, &shapes, &dup, &["left".to_string()]).is_err());
        // An internal face named as a patch face is an error.
        let internal = vec![vec![vec![1usize, 7, 10, 4]]];
        assert!(mesh_from_shapes(12, &shapes, &internal, &["x".to_string()]).is_err());
    }

    fn grid(n: [usize; 3], origin: Vector3, size: Vector3) -> BlockGrid {
        let mut points = Vec::new();
        for k in 0..=n[2] {
            for j in 0..=n[1] {
                for i in 0..=n[0] {
                    points.push(Vector3::new(
                        origin.x + size.x * i as f64 / n[0] as f64,
                        origin.y + size.y * j as f64 / n[1] as f64,
                        origin.z + size.z * k as f64 / n[2] as f64,
                    ));
                }
            }
        }
        BlockGrid { n, points }
    }

    #[test]
    fn block_grid_addressing() {
        let g = grid([2, 3, 1], Vector3::ZERO, Vector3::new(2.0, 3.0, 1.0));
        assert_eq!(g.n_points(), 3 * 4 * 2);
        assert_eq!(g.n_cells(), 6);
        assert_eq!(g.cells().len(), 6);
        let b = g.boundary_patches();
        assert_eq!(b[0].len(), 3);
        assert_eq!(b[1].len(), 3);
        assert_eq!(b[2].len(), 2);
        assert_eq!(b[3].len(), 2);
        assert_eq!(b[4].len(), 6);
        assert_eq!(b[5].len(), 6);
        // Cell 0 is (0,0,0); its first corner is point 0, its 6th is (1,1,1).
        assert_eq!(g.cells()[0][0], 0);
        assert_eq!(g.cells()[0][6], g.point_label(1, 1, 1));
    }

    #[test]
    fn two_block_merge_numbers_points_by_first_appearance() {
        // Two 2x1x1 blocks sharing x=2: 3*2*2 + 3*2*2 = 24 block points,
        // 4 shared, so 20 merged points; block 1's shared points take
        // block 0's numbers.
        let g0 = grid([2, 1, 1], Vector3::ZERO, Vector3::new(2.0, 1.0, 1.0));
        let g1 = grid([2, 1, 1], Vector3::new(2.0, 0.0, 0.0), Vector3::new(2.0, 1.0, 1.0));
        let hex = |o: usize| CellShape {
            model: CellModel::Hex,
            labels: [0, 1, 2, 3, 4, 5, 6, 7].iter().map(|l| l + o).collect(),
        };
        // Topology: vertices of the two blocks (12 points of a 3x2x2 grid).
        let topo_pts: Vec<Vector3> = (0..12)
            .map(|n| Vector3::new((n % 3) as f64 * 2.0, ((n / 3) % 2) as f64, (n / 6) as f64))
            .collect();
        let shapes = two_hex_labels();
        let topo = mesh_from_shapes(12, &shapes, &[], &[]).unwrap();
        let _ = hex;
        let merge = calc_merge_info(&[g0.clone(), g1.clone()], &topo, &topo_pts, true).unwrap();
        assert_eq!(merge.n_points, 20);
        assert_eq!(merge.n_cells, 4);
        // Block 0's points are 0..12, block 1's first (x=2) column reuses them.
        let ml = &merge.merge_list;
        assert_eq!(&ml[..12], &(0..12).collect::<Vec<_>>()[..]);
        assert_eq!(ml[merge.block_offsets[1]], 2); // (2,0,0) is block 0's i=2 point
        assert_eq!(ml[merge.block_offsets[1] + 1], 12); // first new point
        let pts = create_points(&[g0, g1], &merge, 1.0);
        assert_eq!(pts.len(), 20);
        let cells = create_cells(&[grid([2, 1, 1], Vector3::ZERO, Vector3::new(2.0, 1.0, 1.0))], &MergeInfo {
            block_offsets: vec![0],
            merge_list: (0..12).collect(),
            n_points: 12,
            n_cells: 2,
        })
        .unwrap();
        assert_eq!(cells.len(), 2);
        assert!(cells.iter().all(|c| c.model == CellModel::Hex));
    }
}
