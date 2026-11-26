// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OpenFOAM-ESI v2506 (the mesh-to-mesh mapper GeN-Foam's meshHandler uses)
//   Upstream: https://develop.openfoam.com/Development/openfoam (tag OpenFOAM-v2506)
//   Upstream source:
//     src/sampling/meshToMesh/calcMethod/cellVolumeWeight/cellVolumeWeightMethod.C
//         (findInitialSeeds, calculateAddressing, setNextCells, calculate)
//     src/sampling/meshToMesh/calcMethod/meshToMeshMethod/meshToMeshMethod.C
//         (tolerance_, maskCells, intersect, interVol, appendNbrCells, initialise)
//     src/meshTools/tetOverlapVolume/tetOverlapVolume.{C,H}, tetOverlapVolumeTemplates.C
//         (tetTetOverlap, cellCellOverlapMinDecomp, hasOverlapOp, sumOverlapOp, pyrBb)
//     src/OpenFOAM/meshes/primitiveShapes/tetrahedron/tetrahedronI.H
//         (tetSliceWithPlane, planeIntersection, decomposePrism, mag, bounds, tri, inside)
//     src/OpenFOAM/meshes/primitiveShapes/plane/plane.C, planeI.H (signedDistance)
//     src/OpenFOAM/meshes/primitiveMesh (cells, cellCells, cellBb), boundBox{,I}.H
//     src/sampling/meshToMesh/meshToMesh.C (calculate, normaliseWeights)
//     src/sampling/meshToMesh/meshToMeshTemplates.C (mapSrcToTgt, mapTgtToSrc)
//   Called from: GeN-Foam src/classes/multiRegion/meshHandler/meshHandler.C:192-201 and
//                meshHandlerTemplates.C:85,133
//   Upstream copyright: (C) 2011-2017 OpenFOAM Foundation, (C) 2015-2023 OpenCFD Ltd.
//   Upstream license: GPL-3.0-or-later
//   Independent cross-reference (NOT used by this file, outram-blender is read-only here and orders
//   the floating-point work differently): crates/outram-blender/src/unstructured/overlap.rs.
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
//
// This offering is not approved or endorsed by the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # `meshToMesh` with `imCellVolumeWeight`: exact cell-overlap addressing and weights (gh:#651)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! GeN-Foam builds `meshToMesh(src, tgt, imCellVolumeWeight, pmAABB, false)` between its region meshes
//! (`meshHandler.C:192-201`) and moves fields with `mapTgtToSrc(field, plusEqOp, result)`
//! (`meshHandlerTemplates.C:85,133`). This module is that operator, translated line by line from OpenFOAM-ESI v2506,
//! in the same order of floating-point operations, so that the addressing and the weights agree with the real code
//! bit for bit (V&V: README `Round 28`, `tests/genfoam_mesh_to_mesh_vs_upstream.rs`).
//!
//! ## What it computes
//!
//! Per pair of cells `(src s, tgt t)` the exact volume of the intersection of the two polyhedra, summed over the pairs of
//! tetrahedra of their decompositions (`tetOverlapVolume`): each cell is cut into tets `(cell centre, face base point
//! f[0], f[i], f[i+1])`, a tet of one cell is sliced in turn by the four face planes of a tet of the other
//! (`tetrahedron::sliceWithPlane`), and the inside pieces' volumes are summed. A pair is kept when its volume exceeds
//! `1e-6` of the SOURCE cell's volume (`meshToMeshMethod::tolerance_`). The pairs are found by an advancing front over
//! the target cells (`cellVolumeWeightMethod::calculateAddressing`), which fixes the ORDER of the entries and therefore
//! the order of every sum below. [`CellVolumeWeightAddressing::normalise`] divides each cell's weights by their sum
//! (`meshToMesh::normaliseWeights`, which `meshToMesh.C:calculate(.., normalise = true)` applies on the path GeN-Foam
//! takes); a target cell is then the overlap-volume-weighted MEAN of the source cells it intersects.
//!
//! [`CellVolumeWeightAddressing::map_tgt_to_src_scalar`] is `meshToMesh::mapTgtToSrc` with `plusEqOp`: for every cell that
//! has addresses, `result *= (1 - sum w)` and then `result += w * value` per address in order; a cell with no address keeps
//! its value. There is NO global rescale.
//!
//! ## Which object maps which way (the part that is easy to get backwards)
//!
//! `mappings { A { B { sourceFields f; targetFields g; } } }` makes `mappingList_[A][B] = meshToMesh(src = A, tgt = B)` and
//! moves `f` (living on B) onto `g` (living on A) with `mapTgtToSrc`. The upstream object is the one whose SOURCE mesh is
//! the region that RECEIVES the field, and it uses `srcToTgtCellAddr_/Wght_`. [`CellVolumeWeightAddressing::calculate`]
//! takes `(src, tgt)` in upstream's sense.
//!
//! ## Deviations from upstream (stated, with their consequence)
//!
//! * `findInitialSeeds` asks upstream's octree (`cellTree().findInside(pt)`, `CELL_TETS` decomposition with minimum-twist
//!   base points). Here the first target cell, in ascending index, whose bounding box holds the point and whose fan
//!   decomposition `(centre, f[0], f[i], f[i+1])` holds it (`tetrahedron::inside`, tolerance `SMALL`) is taken. It only
//!   chooses WHERE the front starts; on the 3D_SmallESFR meshes it gives the same addressing order (README Round 28).
//! * Serial only (`singleMeshProc_`), as in the other ports; no `distributed()` branch.
//! * The `mapSrcToTgt`/`mapTgtToSrc` second-order (`tgtToSrcCellVec_`) and AMI patch branches are not ported (GeN-Foam
//!   passes `interpAllPatches = false` and `secondOrder = false`).
//! * Tets produced by slicing are held in growable vectors, not upstream's fixed `FixedList<tetPoints, 200>`; a case that
//!   would overflow upstream's list (it does not abort there either, it writes past the end) simply works.
//!
//! ## Units
//!
//! Points are plain `f64` metres, volumes m^3, as in the rest of `multi_region`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use outram_foam_basic_lib::prelude::Vector3;

use crate::io::poly_mesh::{compute_cell_geometry, compute_face_geometry};

type P = [f64; 3];

/// `meshToMeshMethod::tolerance_`: an overlap below this fraction of the source cell volume is not an overlap.
pub const OVERLAP_TOLERANCE: f64 = 1.0e-6;
/// `ROOTVSMALL` of a double build (`1e-150`).
const ROOT_V_SMALL: f64 = 1.0e-150;
/// `SMALL` of a double build (`1e-15`).
const SMALL: f64 = 1.0e-15;

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `scalar * vector`, component by component (`operator*(scalar, Vector)`).
fn scale(s: f64, v: P) -> P {
    [s * v[0], s * v[1], s * v[2]]
}

/// `vector / scalar`, component by component (a division per component, not a multiplication by the reciprocal).
fn div(v: P, s: f64) -> P {
    [v[0] / s, v[1] / s, v[2] / s]
}

/// `operator^` of `Vector`.
fn cross(a: P, b: P) -> P {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// `operator&` of `Vector`.
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `magSqr(Vector)`.
fn mag_sqr(a: P) -> f64 {
    a[0] * a[0] + a[1] * a[1] + a[2] * a[2]
}

/// An axis-aligned box (`boundBox`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BBox {
    /// Minimum corner.
    pub min: [f64; 3],
    /// Maximum corner.
    pub max: [f64; 3],
}

impl BBox {
    /// A box holding the single point `p` (`treeBoundBox(fc)`).
    fn at(p: P) -> Self {
        BBox { min: p, max: p }
    }

    /// `boundBox::add(point)`: componentwise `min` / `max`.
    fn add(&mut self, p: P) {
        for k in 0..3 {
            if p[k] < self.min[k] {
                self.min[k] = p[k];
            }
            if p[k] > self.max[k] {
                self.max[k] = p[k];
            }
        }
    }

    /// `boundBox::overlaps(bb)` = `box_box_overlaps` (closed boxes: touching counts).
    pub fn overlaps(&self, o: &BBox) -> bool {
        self.min[0] <= o.max[0]
            && o.min[0] <= self.max[0]
            && self.min[1] <= o.max[1]
            && o.min[1] <= self.max[1]
            && self.min[2] <= o.max[2]
            && o.min[2] <= self.max[2]
    }

    /// Whether the box holds the point (inclusive).
    fn contains(&self, p: P) -> bool {
        (0..3).all(|k| self.min[k] <= p[k] && p[k] <= self.max[k])
    }
}

/// A polyhedral mesh with exactly the data `meshToMesh` reads from a `polyMesh`: points, faces, owner, neighbour, and the
/// derived cell-face lists, cell-cell lists, cell centres, cell volumes and bounding boxes.
#[derive(Debug, Clone)]
pub struct OverlapMesh {
    points: Vec<P>,
    faces: Vec<Vec<usize>>,
    owner: Vec<usize>,
    n_cells: usize,
    /// `primitiveMesh::cells()`: owner faces ascending, then neighbour faces ascending.
    cells: Vec<Vec<usize>>,
    /// `primitiveMesh::cellCells()`.
    cell_cells: Vec<Vec<usize>>,
    cell_centres: Vec<P>,
    cell_volumes: Vec<f64>,
    cell_bb: Vec<BBox>,
    bounds: BBox,
}

/// Errors building an [`OverlapMesh`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlapMeshError {
    /// Inconsistent lists (named).
    Invalid(String),
}

impl core::fmt::Display for OverlapMeshError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            OverlapMeshError::Invalid(m) => write!(f, "overlap mesh: {m}"),
        }
    }
}

impl core::error::Error for OverlapMeshError {}

impl OverlapMesh {
    /// Build from raw `polyMesh` lists. The cell centres and volumes come from the same routines as
    /// [`DeformableMesh`](super::mesh_deformation::DeformableMesh) (bit-identical to ESI, gh:#1489, round 26).
    ///
    /// # Errors
    /// [`OverlapMeshError::Invalid`] if the lists disagree in size or a face names a point that does not exist.
    pub fn new(
        points: &[Vector3],
        faces: Vec<Vec<usize>>,
        owner: Vec<usize>,
        neighbour: Vec<usize>,
    ) -> Result<Self, OverlapMeshError> {
        if owner.len() != faces.len() || neighbour.len() > faces.len() {
            return Err(OverlapMeshError::Invalid("owner/neighbour sizes do not match the faces".into()));
        }
        if faces.iter().any(|f| f.len() < 3 || f.iter().any(|&p| p >= points.len())) {
            return Err(OverlapMeshError::Invalid("a face has fewer than 3 points or names a point that does not exist".into()));
        }
        let n_cells = owner.iter().chain(neighbour.iter()).copied().max().map_or(0, |m| m + 1);
        let raw: Vec<[f64; 3]> = points.iter().map(|p| [p.x, p.y, p.z]).collect();
        let (face_centres, face_area_vectors, _areas) =
            compute_face_geometry(&raw, &faces, "faces").map_err(|e| OverlapMeshError::Invalid(e.to_string()))?;
        let (cell_volumes, centres) = compute_cell_geometry(&face_centres, &face_area_vectors, &owner, &neighbour, n_cells);
        let cell_centres: Vec<P> = centres.iter().map(|c| [c.x, c.y, c.z]).collect();

        // primitiveMesh::calcCells(cellFaceAddr, own, nei, nCells)
        let mut cells: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
        for (f, &o) in owner.iter().enumerate() {
            cells[o].push(f);
        }
        for (f, &n) in neighbour.iter().enumerate() {
            cells[n].push(f);
        }
        // primitiveMesh::calcCellCells()
        let mut cell_cells: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
        for (f, &n) in neighbour.iter().enumerate() {
            let o = owner[f];
            cell_cells[o].push(n);
            cell_cells[n].push(o);
        }
        // primitiveMesh::cellBb: the min / max of the cell's points (order does not matter)
        let mut cell_bb: Vec<BBox> = Vec::with_capacity(n_cells);
        for c in 0..n_cells {
            let mut bb: Option<BBox> = None;
            for &f in &cells[c] {
                for &p in &faces[f] {
                    match bb.as_mut() {
                        Some(b) => b.add(raw[p]),
                        None => bb = Some(BBox::at(raw[p])),
                    }
                }
            }
            cell_bb.push(bb.unwrap_or(BBox::at([0.0; 3])));
        }
        // polyMesh::bounds(): the box of all points
        let mut bounds = BBox::at(raw.first().copied().unwrap_or([0.0; 3]));
        for p in &raw {
            bounds.add(*p);
        }
        Ok(OverlapMesh { points: raw, faces, owner, n_cells, cells, cell_cells, cell_centres, cell_volumes, cell_bb, bounds })
    }

    /// The polyhedral mesh of a [`DeformableMesh`](super::mesh_deformation::DeformableMesh) at its CURRENT points (so a
    /// mesh moved by `deformMesh()` overlaps the undeformed one, the use-case of gh:#1852).
    ///
    /// # Errors
    /// As [`OverlapMesh::new`].
    pub fn from_deformable(mesh: &super::mesh_deformation::DeformableMesh) -> Result<Self, OverlapMeshError> {
        let t = mesh.topology();
        Self::new(mesh.points(), t.faces().to_vec(), t.owner().to_vec(), t.neighbour().to_vec())
    }

    /// An `nx x ny x nz` block of hexahedra between the corners `lo` and `hi` (a `blockMesh`-like mesh for tests and
    /// small cases): internal faces first, boundary faces after, every face's points ordered so its normal leaves the owner.
    ///
    /// # Panics
    /// Panics if a count is zero (a block needs at least one cell per direction).
    #[must_use]
    pub fn box_grid(nx: usize, ny: usize, nz: usize, lo: [f64; 3], hi: [f64; 3]) -> Self {
        assert!(nx > 0 && ny > 0 && nz > 0, "a block needs at least one cell per direction");
        let pt = |i: usize, j: usize, k: usize| (k * (ny + 1) + j) * (nx + 1) + i;
        let mut points = Vec::with_capacity((nx + 1) * (ny + 1) * (nz + 1));
        for k in 0..=nz {
            for j in 0..=ny {
                for i in 0..=nx {
                    points.push(Vector3::new(
                        lo[0] + (hi[0] - lo[0]) * (i as f64) / (nx as f64),
                        lo[1] + (hi[1] - lo[1]) * (j as f64) / (ny as f64),
                        lo[2] + (hi[2] - lo[2]) * (k as f64) / (nz as f64),
                    ));
                }
            }
        }
        let cell = |i: usize, j: usize, k: usize| (k * ny + j) * nx + i;
        let mut faces: Vec<Vec<usize>> = Vec::new();
        let mut owner: Vec<usize> = Vec::new();
        let mut neighbour: Vec<usize> = Vec::new();
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let c = cell(i, j, k);
                    if i + 1 < nx {
                        faces.push(vec![pt(i + 1, j, k), pt(i + 1, j + 1, k), pt(i + 1, j + 1, k + 1), pt(i + 1, j, k + 1)]);
                        owner.push(c);
                        neighbour.push(cell(i + 1, j, k));
                    }
                    if j + 1 < ny {
                        faces.push(vec![pt(i, j + 1, k), pt(i, j + 1, k + 1), pt(i + 1, j + 1, k + 1), pt(i + 1, j + 1, k)]);
                        owner.push(c);
                        neighbour.push(cell(i, j + 1, k));
                    }
                    if k + 1 < nz {
                        faces.push(vec![pt(i, j, k + 1), pt(i + 1, j, k + 1), pt(i + 1, j + 1, k + 1), pt(i, j + 1, k + 1)]);
                        owner.push(c);
                        neighbour.push(cell(i, j, k + 1));
                    }
                }
            }
        }
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let c = cell(i, j, k);
                    if i == 0 {
                        faces.push(vec![pt(i, j, k), pt(i, j, k + 1), pt(i, j + 1, k + 1), pt(i, j + 1, k)]);
                        owner.push(c);
                    }
                    if i + 1 == nx {
                        faces.push(vec![pt(i + 1, j, k), pt(i + 1, j + 1, k), pt(i + 1, j + 1, k + 1), pt(i + 1, j, k + 1)]);
                        owner.push(c);
                    }
                    if j == 0 {
                        faces.push(vec![pt(i, j, k), pt(i + 1, j, k), pt(i + 1, j, k + 1), pt(i, j, k + 1)]);
                        owner.push(c);
                    }
                    if j + 1 == ny {
                        faces.push(vec![pt(i, j + 1, k), pt(i, j + 1, k + 1), pt(i + 1, j + 1, k + 1), pt(i + 1, j + 1, k)]);
                        owner.push(c);
                    }
                    if k == 0 {
                        faces.push(vec![pt(i, j, k), pt(i, j + 1, k), pt(i + 1, j + 1, k), pt(i + 1, j, k)]);
                        owner.push(c);
                    }
                    if k + 1 == nz {
                        faces.push(vec![pt(i, j, k + 1), pt(i + 1, j, k + 1), pt(i + 1, j + 1, k + 1), pt(i, j + 1, k + 1)]);
                        owner.push(c);
                    }
                }
            }
        }
        // by construction the lists are consistent
        OverlapMesh::new(&points, faces, owner, neighbour).expect("box_grid builds consistent lists")
    }

    /// Number of cells.
    #[must_use]
    pub fn n_cells(&self) -> usize {
        self.n_cells
    }

    /// Cell volumes `V` (m^3).
    #[must_use]
    pub fn cell_volumes(&self) -> &[f64] {
        &self.cell_volumes
    }

    /// Cell centres.
    #[must_use]
    pub fn cell_centres(&self) -> Vec<Vector3> {
        self.cell_centres.iter().map(|c| Vector3::new(c[0], c[1], c[2])).collect()
    }

    /// `cell::points(faces, meshPoints)`: the cell's distinct points in order of first appearance over its faces.
    fn cell_points_ordered(&self, c: usize) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        for &f in &self.cells[c] {
            for &p in &self.faces[f] {
                if !out.contains(&p) {
                    out.push(p);
                }
            }
        }
        out
    }

    /// `pyrBb(points, f, fc)`: the box of a face and the cell centre.
    fn pyr_bb(&self, f: usize, fc: P) -> BBox {
        let mut bb = BBox::at(fc);
        for &p in &self.faces[f] {
            bb.add(self.points[p]);
        }
        bb
    }

    /// Whether `p` is inside cell `c`: some tet of the face fans `(centre, f[0], f[i], f[i+1])` holds it
    /// (see the module docs, deviation 1).
    fn point_in_cell(&self, p: P, c: usize) -> bool {
        let cc = self.cell_centres[c];
        for &f in &self.cells[c] {
            let face = &self.faces[f];
            let base = self.points[face[0]];
            for i in 1..face.len() - 1 {
                let t = [cc, base, self.points[face[i]], self.points[face[i + 1]]];
                if tet_inside(&t, p) {
                    return true;
                }
            }
        }
        false
    }

    /// `cellTree().findInside(pt)` in the sense of the module docs: the lowest cell index holding the point.
    fn find_inside(&self, p: P) -> Option<usize> {
        (0..self.n_cells).find(|&c| self.cell_bb[c].contains(p) && self.point_in_cell(p, c))
    }
}

/// `tetrahedron::inside(p)` with `SMALL`, written orientation-independently: `p` is outside when it lies further than
/// `SMALL` beyond any face plane, on the side away from the opposite vertex.
fn tet_inside(t: &[P; 4], p: P) -> bool {
    const FACE: [[usize; 4]; 4] = [[1, 2, 3, 0], [0, 3, 2, 1], [0, 1, 3, 2], [0, 2, 1, 3]];
    for f in FACE {
        let (a, b, c, opp) = (t[f[0]], t[f[1]], t[f[2]], t[f[3]]);
        let n = cross(sub(b, a), sub(c, a));
        let m = mag_sqr(n).sqrt();
        if m <= ROOT_V_SMALL {
            continue;
        }
        let mut u = div(n, m);
        if dot(sub(opp, a), u) > 0.0 {
            u = [-u[0], -u[1], -u[2]];
        }
        if dot(sub(p, a), u) > SMALL {
            return false;
        }
    }
    true
}

/// `tetrahedron::mag()` (signed): `(1/6) ((b - a) ^ (c - a)) & (d - a)`.
fn tet_mag(t: &[P; 4]) -> f64 {
    (1.0 / 6.0) * dot(cross(sub(t[1], t[0]), sub(t[2], t[0])), sub(t[3], t[0]))
}

/// `tetrahedron::tri(facei).areaNormal()` = `0.5 ((b - a) ^ (c - a))` of the face triangle.
fn tet_face_area_normal(t: &[P; 4], face: usize) -> P {
    let (a, b, c) = match face {
        0 => (t[1], t[2], t[3]),
        1 => (t[0], t[3], t[2]),
        2 => (t[0], t[1], t[3]),
        _ => (t[0], t[2], t[1]),
    };
    let n = cross(sub(b, a), sub(c, a));
    [0.5 * n[0], 0.5 * n[1], 0.5 * n[2]]
}

/// `tetPoints::box()`: the componentwise min and max of the four points.
fn tet_box(t: &[P; 4]) -> BBox {
    let mut bb = BBox::at(t[0]);
    for p in &t[1..] {
        bb.add(*p);
    }
    bb
}

/// `tetrahedron::planeIntersection(d, t, negI, posI)`.
fn plane_intersection(d: &[f64; 4], t: &[P; 4], neg: usize, pos: usize) -> P {
    div(sub(scale(d[pos], t[neg]), scale(d[neg], t[pos])), -d[neg] + d[pos])
}

/// `tetrahedron::decomposePrism`.
fn decompose_prism(p: &[P; 6], out: &mut Vec<[P; 4]>) {
    out.push([p[1], p[3], p[2], p[0]]);
    out.push([p[1], p[2], p[3], p[4]]);
    out.push([p[4], p[2], p[3], p[5]]);
}

/// `tetrahedron::tetSliceWithPlane` keeping only the tets ABOVE the plane (`aboveOp`), in upstream's order; the below
/// part is a `dummyOp` in every use of `tetTetOverlap`. `plane` is `(origin, unit normal)`; the distance is
/// `(p - origin) & normal` (`plane::signedDistance`).
fn slice_above(plane: &(P, P), tet: &[P; 4], above: &mut Vec<[P; 4]>) {
    let d: [f64; 4] = [
        dot(sub(tet[0], plane.0), plane.1),
        dot(sub(tet[1], plane.0), plane.1),
        dot(sub(tet[2], plane.0), plane.1),
        dot(sub(tet[3], plane.0), plane.1),
    ];
    let n_pos = d.iter().filter(|&&x| x > 0.0).count();
    let fc = |i: usize| (i + 1) % 4;
    match n_pos {
        4 => above.push(*tet),
        3 => {
            // below tet at the first non-positive vertex, above prism
            let i0 = (0..4).find(|&i| d[i] <= 0.0).unwrap_or(0);
            let i1 = fc(i0);
            let i2 = fc(i1);
            let i3 = fc(i2);
            let p01 = plane_intersection(&d, tet, i0, i1);
            let p02 = plane_intersection(&d, tet, i0, i2);
            let p03 = plane_intersection(&d, tet, i0, i3);
            let prism = if i0 == 0 || i0 == 2 {
                [tet[i1], tet[i3], tet[i2], p01, p03, p02]
            } else {
                [tet[i3], tet[i1], tet[i2], p03, p01, p02]
            };
            decompose_prism(&prism, above);
        }
        2 => {
            let mut pos = (0..4).filter(|&i| d[i] > 0.0);
            let pos0 = pos.next().unwrap_or(0);
            let pos1 = pos.next().unwrap_or(1);
            // the "above" prism of each of the six edges, as listed in tetSliceWithPlane
            let prism: [P; 6] = match (pos0, pos1) {
                (0, 1) => {
                    let p02 = plane_intersection(&d, tet, 0, 2);
                    let p03 = plane_intersection(&d, tet, 0, 3);
                    let p12 = plane_intersection(&d, tet, 1, 2);
                    let p13 = plane_intersection(&d, tet, 1, 3);
                    [tet[0], p02, p03, tet[1], p12, p13]
                }
                (1, 2) => {
                    let p01 = plane_intersection(&d, tet, 0, 1);
                    let p13 = plane_intersection(&d, tet, 1, 3);
                    let p02 = plane_intersection(&d, tet, 0, 2);
                    let p23 = plane_intersection(&d, tet, 2, 3);
                    [tet[1], p01, p13, tet[2], p02, p23]
                }
                (0, 2) => {
                    let p01 = plane_intersection(&d, tet, 0, 1);
                    let p03 = plane_intersection(&d, tet, 0, 3);
                    let p12 = plane_intersection(&d, tet, 1, 2);
                    let p23 = plane_intersection(&d, tet, 2, 3);
                    [tet[2], p12, p23, tet[0], p01, p03]
                }
                (0, 3) => {
                    let p01 = plane_intersection(&d, tet, 0, 1);
                    let p02 = plane_intersection(&d, tet, 0, 2);
                    let p13 = plane_intersection(&d, tet, 1, 3);
                    let p23 = plane_intersection(&d, tet, 2, 3);
                    [tet[3], p23, p13, tet[0], p02, p01]
                }
                (1, 3) => {
                    let p01 = plane_intersection(&d, tet, 0, 1);
                    let p12 = plane_intersection(&d, tet, 1, 2);
                    let p03 = plane_intersection(&d, tet, 0, 3);
                    let p23 = plane_intersection(&d, tet, 2, 3);
                    [tet[1], p12, p01, tet[3], p23, p03]
                }
                _ => {
                    // (2, 3)
                    let p02 = plane_intersection(&d, tet, 0, 2);
                    let p12 = plane_intersection(&d, tet, 1, 2);
                    let p03 = plane_intersection(&d, tet, 0, 3);
                    let p13 = plane_intersection(&d, tet, 1, 3);
                    [tet[2], p02, p12, tet[3], p03, p13]
                }
            };
            decompose_prism(&prism, above);
        }
        1 => {
            let i0 = (0..4).find(|&i| d[i] > 0.0).unwrap_or(0);
            let i1 = fc(i0);
            let i2 = fc(i1);
            let i3 = fc(i2);
            let p01 = plane_intersection(&d, tet, i0, i1);
            let p02 = plane_intersection(&d, tet, i0, i2);
            let p03 = plane_intersection(&d, tet, i0, i3);
            if i0 == 0 || i0 == 2 {
                above.push([tet[i0], p01, p02, p03]);
            } else {
                above.push([p01, p02, p03, tet[i0]]);
            }
        }
        _ => {}
    }
}

/// Reusable buffers for [`tet_tet_overlap`] (upstream's `static tetIntersectionList`s).
#[derive(Debug, Default)]
struct TetScratch {
    cut_inside: Vec<[P; 4]>,
    inside: Vec<[P; 4]>,
    last: Vec<[P; 4]>,
}

/// `tetOverlapVolume::tetTetOverlap<sumVolOp>`: slice `tet_a` by the four face planes of `tet_b` and add the signed
/// volume of every surviving tet to `vol`, in upstream's order (`sumVolOp::vol_ += tet.mag()`).
fn tet_tet_overlap(tet_a: &[P; 4], tet_b: &[P; 4], vol: &mut f64, s: &mut TetScratch) {
    // exit early if any face area of either tet is too small
    for face in 0..4 {
        let n = tet_face_area_normal(tet_a, face);
        if mag_sqr([-n[0], -n[1], -n[2]]) < ROOT_V_SMALL {
            return;
        }
    }
    let mut area_b = [[0.0; 3]; 4];
    let mut mag2_b = [0.0; 4];
    for face in 0..4 {
        let n = tet_face_area_normal(tet_b, face);
        area_b[face] = [-n[0], -n[1], -n[2]];
        mag2_b[face] = mag_sqr(area_b[face]);
        if mag2_b[face] < ROOT_V_SMALL {
            return;
        }
    }
    // the plane of face `f` of B: origin tri(f).a(), unit normal `-areaNormal / sqrt(magSqr)`
    let origin = |f: usize| if f == 0 { tet_b[1] } else { tet_b[0] };
    let plane = |f: usize| (origin(f), div(area_b[f], mag2_b[f].sqrt()));

    // Face 0: A -> cutInside
    s.cut_inside.clear();
    slice_above(&plane(0), tet_a, &mut s.cut_inside);
    if s.cut_inside.is_empty() {
        return;
    }
    // Face 1: cutInside -> inside
    s.inside.clear();
    let pl1 = plane(1);
    for i in 0..s.cut_inside.len() {
        let t = s.cut_inside[i];
        slice_above(&pl1, &t, &mut s.inside);
    }
    if s.inside.is_empty() {
        return;
    }
    // Face 2: inside -> cutInside
    s.cut_inside.clear();
    let pl2 = plane(2);
    for i in 0..s.inside.len() {
        let t = s.inside[i];
        slice_above(&pl2, &t, &mut s.cut_inside);
    }
    if s.cut_inside.is_empty() {
        return;
    }
    // Face 3: cutInside -> the summing op, one tet at a time in order
    let pl3 = plane(3);
    for i in 0..s.cut_inside.len() {
        let t = s.cut_inside[i];
        s.last.clear();
        slice_above(&pl3, &t, &mut s.last);
        for r in &s.last {
            *vol += tet_mag(r);
        }
    }
}

/// What `cellCellOverlapMinDecomp` does with each pair of tets.
#[derive(Debug, Clone, Copy)]
enum OverlapOp {
    /// `sumOverlapOp`: add up every pair.
    Sum,
    /// `hasOverlapOp(threshold)`: add up, stop as soon as the running volume exceeds the threshold.
    Has(f64),
}

/// `tetOverlapVolume::cellCellOverlapMinDecomp`: the (possibly early-stopped) running overlap volume of cell `ca` of
/// `a` and cell `cb` of `b`. `bb_b` is `b.cellBb(cb)`. Returns `(volume, ok)`, `ok` being `hasOverlapOp::ok_`.
fn cell_cell_overlap(
    a: &OverlapMesh,
    ca: usize,
    b: &OverlapMesh,
    cb: usize,
    bb_b: &BBox,
    op: OverlapOp,
    s: &mut TetScratch,
) -> (f64, bool) {
    let mut vol = 0.0;
    let cc_a = a.cell_centres[ca];
    let cc_b = b.cell_centres[cb];
    // pyrB of every face of B, computed once (a pure function of the face)
    let pyr_b: Vec<BBox> = b.cells[cb].iter().map(|&f| b.pyr_bb(f, cc_b)).collect();

    for &face_a in &a.cells[ca] {
        let f_a = &a.faces[face_a];
        let pyr_a = a.pyr_bb(face_a, cc_a);
        if !pyr_a.overlaps(bb_b) {
            continue;
        }
        let own_a = a.owner[face_a] == ca;
        let base_a = a.points[f_a[0]];
        for tet_pt in 1..f_a.len() - 1 {
            let face_pt = tet_pt % f_a.len();
            let other = (face_pt + 1) % f_a.len();
            let (p0, p1) = if own_a { (f_a[face_pt], f_a[other]) } else { (f_a[other], f_a[face_pt]) };
            let tet_a = [cc_a, base_a, a.points[p0], a.points[p1]];
            let tet_a_bb = tet_box(&tet_a);

            for (k, &face_b) in b.cells[cb].iter().enumerate() {
                if !pyr_b[k].overlaps(&pyr_a) {
                    continue;
                }
                let f_b = &b.faces[face_b];
                let own_b = b.owner[face_b] == cb;
                let base_b = b.points[f_b[0]];
                for tet_pt_b in 1..f_b.len() - 1 {
                    let face_pt_b = tet_pt_b % f_b.len();
                    let other_b = (face_pt_b + 1) % f_b.len();
                    let (q0, q1) = if own_b { (f_b[face_pt_b], f_b[other_b]) } else { (f_b[other_b], f_b[face_pt_b]) };
                    let tet_b = [cc_b, base_b, b.points[q0], b.points[q1]];
                    let tet_b_bb = tet_box(&tet_b);
                    if !tet_b_bb.overlaps(&tet_a_bb) {
                        continue;
                    }
                    tet_tet_overlap(&tet_a, &tet_b, &mut vol, s);
                    if let OverlapOp::Has(threshold) = op {
                        if vol > threshold {
                            return (vol, true);
                        }
                    }
                }
            }
        }
    }
    (vol, false)
}

/// `DynamicList::push_uniq`.
fn push_uniq(list: &mut Vec<usize>, v: usize) {
    if !list.contains(&v) {
        list.push(v);
    }
}

/// `meshToMeshMethod::appendNbrCells`.
fn append_nbr_cells(c: usize, mesh: &OverlapMesh, visited: &[usize], nbrs: &mut Vec<usize>) {
    for &n in &mesh.cell_cells[c] {
        if !visited.contains(&n) {
            push_uniq(nbrs, n);
        }
    }
}

/// The addressing and weights of a `meshToMesh` (`srcToTgtCellAddr_`, `srcToTgtCellWght_`, `tgtToSrcCellAddr_`,
/// `tgtToSrcCellWght_`, `V_`), in upstream's order. Right after [`calculate`](Self::calculate) the weights are the raw overlap
/// volumes; after [`normalise`](Self::normalise) they are divided by their per-cell sum.
#[derive(Debug, Clone, PartialEq)]
pub struct CellVolumeWeightAddressing {
    /// `srcToTgtCellAddr_[src]`: target cells overlapping source cell `src`, in the order the front found them.
    pub src_to_tgt_addr: Vec<Vec<usize>>,
    /// `srcToTgtCellWght_[src]`.
    pub src_to_tgt_wght: Vec<Vec<f64>>,
    /// `tgtToSrcCellAddr_[tgt]`.
    pub tgt_to_src_addr: Vec<Vec<usize>>,
    /// `tgtToSrcCellWght_[tgt]`.
    pub tgt_to_src_wght: Vec<Vec<f64>>,
    /// `V_`: the total overlap volume, accumulated in the front's order.
    pub v: f64,
}

/// `sum(UList<scalar>)`: `0 + w[0] + w[1] + ...` left to right.
fn sum_list(w: &[f64]) -> f64 {
    let mut s = 0.0;
    for &x in w {
        s += x;
    }
    s
}

impl CellVolumeWeightAddressing {
    /// `cellVolumeWeightMethod::calculate(src, tgt)` (raw overlap volumes, not yet normalised).
    #[must_use]
    pub fn calculate(src: &OverlapMesh, tgt: &OverlapMesh) -> Self {
        let mut out = CellVolumeWeightAddressing {
            src_to_tgt_addr: vec![Vec::new(); src.n_cells],
            src_to_tgt_wght: vec![Vec::new(); src.n_cells],
            tgt_to_src_addr: vec![Vec::new(); tgt.n_cells],
            tgt_to_src_wght: vec![Vec::new(); tgt.n_cells],
            v: 0.0,
        };
        // initialise(): nothing to do with an empty mesh
        if src.n_cells == 0 || tgt.n_cells == 0 {
            return out;
        }
        let mut scratch = TetScratch::default();

        // maskCells(): the source cells whose box meets the (inflated) intersection of the two mesh boxes
        let mut ib = BBox {
            min: [0.0; 3],
            max: [0.0; 3],
        };
        for k in 0..3 {
            ib.min[k] = if src.bounds.min[k] > tgt.bounds.min[k] { src.bounds.min[k] } else { tgt.bounds.min[k] };
            ib.max[k] = if src.bounds.max[k] < tgt.bounds.max[k] { src.bounds.max[k] } else { tgt.bounds.max[k] };
        }
        let delta = 0.01 * mag_sqr(sub(ib.max, ib.min)).sqrt();
        for k in 0..3 {
            ib.min[k] -= delta;
            ib.max[k] += delta;
        }
        let src_cell_ids: Vec<usize> = (0..src.n_cells).filter(|&c| ib.overlaps(&src.cell_bb[c])).collect();
        let mut map_flag = vec![false; src.n_cells];
        for &c in &src_cell_ids {
            map_flag[c] = true;
        }

        let mut start_seed = 0usize;
        let seed = find_initial_seeds(src, tgt, &src_cell_ids, &map_flag, start_seed, &mut scratch);
        if let Some((src_seed, tgt_seed)) = seed {
            out.calculate_addressing(src, tgt, src_seed, tgt_seed, &src_cell_ids, &mut map_flag, &mut start_seed, &mut scratch);
        }
        out
    }

    /// `cellVolumeWeightMethod::calculateAddressing`.
    #[allow(clippy::too_many_arguments)]
    fn calculate_addressing(
        &mut self,
        src: &OverlapMesh,
        tgt: &OverlapMesh,
        src_seed: usize,
        tgt_seed: usize,
        src_cell_ids: &[usize],
        map_flag: &mut [bool],
        start_seed: &mut usize,
        s: &mut TetScratch,
    ) {
        let mut src_celli = src_seed;
        let mut tgt_celli = tgt_seed;
        let mut queued: Vec<usize> = Vec::new();
        let mut visited: Vec<usize> = Vec::new();
        // tgt cells used to seed src cells
        let mut seed_cells: Vec<Option<usize>> = vec![None; src.n_cells];
        seed_cells[src_celli] = Some(tgt_celli);

        loop {
            queued.clear();
            visited.clear();
            // initial target cell and its neighbours
            queued.push(tgt_celli);
            append_nbr_cells(tgt_celli, tgt, &visited, &mut queued);

            while let Some(t) = queued.pop() {
                // process the new target cell as LIFO
                tgt_celli = t;
                visited.push(tgt_celli);
                let vol = inter_vol(src, src_celli, tgt, tgt_celli, s);
                // accumulate addressing and weights for a valid intersection
                if vol / src.cell_volumes[src_celli] > OVERLAP_TOLERANCE {
                    self.src_to_tgt_addr[src_celli].push(tgt_celli);
                    self.src_to_tgt_wght[src_celli].push(vol);
                    self.tgt_to_src_addr[tgt_celli].push(src_celli);
                    self.tgt_to_src_wght[tgt_celli].push(vol);
                    append_nbr_cells(tgt_celli, tgt, &visited, &mut queued);
                    // accumulate intersection volume
                    self.v += vol;
                }
            }
            map_flag[src_celli] = false;

            // find the new source seed cell
            match set_next_cells(src, tgt, start_seed, src_celli, src_cell_ids, map_flag, &visited, &mut seed_cells, s) {
                Some((sc, tc)) => {
                    src_celli = sc;
                    tgt_celli = tc;
                }
                None => break,
            }
        }
    }

    /// `meshToMesh::normaliseWeights` on both directions: every cell's weights divided by their sum.
    pub fn normalise(&mut self) {
        for w in self.src_to_tgt_wght.iter_mut().chain(self.tgt_to_src_wght.iter_mut()) {
            let s = sum_list(w);
            for x in w.iter_mut() {
                *x /= s;
            }
        }
    }

    /// `meshToMesh(src, tgt, imCellVolumeWeight, pmAABB, false)`: [`calculate`](Self::calculate) then
    /// [`normalise`](Self::normalise) (`normalise` defaults to `true` on that constructor path).
    #[must_use]
    pub fn new_normalised(src: &OverlapMesh, tgt: &OverlapMesh) -> Self {
        let mut a = Self::calculate(src, tgt);
        a.normalise();
        a
    }

    /// `meshToMesh::mapTgtToSrc(tgtField, plusEqOp, result)` for a scalar cell list (`meshToMeshTemplates.C:261`).
    ///
    /// # Panics
    /// Panics if `result` is not sized to the source mesh or `tgt_field` to the target mesh (upstream: `FatalError`).
    pub fn map_tgt_to_src_scalar(&self, tgt_field: &[f64], result: &mut [f64]) {
        assert_eq!(result.len(), self.src_to_tgt_addr.len(), "Supplied field size is not equal to source mesh size");
        assert_eq!(tgt_field.len(), self.tgt_to_src_addr.len(), "target field is not sized to the target mesh");
        for (celli, res) in result.iter_mut().enumerate() {
            let addr = &self.src_to_tgt_addr[celli];
            let wght = &self.src_to_tgt_wght[celli];
            if !addr.is_empty() {
                *res *= 1.0 - sum_list(wght);
                for (i, &tgt) in addr.iter().enumerate() {
                    *res += wght[i] * tgt_field[tgt];
                }
            }
        }
    }

    /// `meshToMesh::mapSrcToTgt(srcField, plusEqOp, result)` for a scalar cell list (`meshToMeshTemplates.C:40`).
    ///
    /// # Panics
    /// Panics if `result` is not sized to the target mesh or `src_field` to the source mesh.
    pub fn map_src_to_tgt_scalar(&self, src_field: &[f64], result: &mut [f64]) {
        assert_eq!(result.len(), self.tgt_to_src_addr.len(), "Supplied field size is not equal to target mesh size");
        assert_eq!(src_field.len(), self.src_to_tgt_addr.len(), "source field is not sized to the source mesh");
        for (celli, res) in result.iter_mut().enumerate() {
            let addr = &self.tgt_to_src_addr[celli];
            let wght = &self.tgt_to_src_wght[celli];
            if !addr.is_empty() {
                *res *= 1.0 - sum_list(wght);
                for (i, &src) in addr.iter().enumerate() {
                    *res += wght[i] * src_field[src];
                }
            }
        }
    }

    /// `mapTgtToSrc` for a vector cell list: the same operations per component (`weight * y` is a per-component product).
    ///
    /// # Panics
    /// As [`map_tgt_to_src_scalar`](Self::map_tgt_to_src_scalar).
    pub fn map_tgt_to_src_vector(&self, tgt_field: &[Vector3], result: &mut [Vector3]) {
        assert_eq!(result.len(), self.src_to_tgt_addr.len(), "Supplied field size is not equal to source mesh size");
        assert_eq!(tgt_field.len(), self.tgt_to_src_addr.len(), "target field is not sized to the target mesh");
        for (celli, res) in result.iter_mut().enumerate() {
            let addr = &self.src_to_tgt_addr[celli];
            let wght = &self.src_to_tgt_wght[celli];
            if !addr.is_empty() {
                let f = 1.0 - sum_list(wght);
                *res = Vector3::new(res.x * f, res.y * f, res.z * f);
                for (i, &tgt) in addr.iter().enumerate() {
                    let w = wght[i];
                    let y = tgt_field[tgt];
                    *res = Vector3::new(res.x + w * y.x, res.y + w * y.y, res.z + w * y.z);
                }
            }
        }
    }
}

/// `meshToMeshMethod::intersect`: whether the cells overlap by more than `1e-6` of the source cell's volume.
fn intersect(src: &OverlapMesh, sc: usize, tgt: &OverlapMesh, tc: usize, s: &mut TetScratch) -> bool {
    let threshold = OVERLAP_TOLERANCE * src.cell_volumes[sc];
    let bb = tgt.cell_bb[tc];
    cell_cell_overlap(src, sc, tgt, tc, &bb, OverlapOp::Has(threshold), s).1
}

/// `meshToMeshMethod::interVol`.
fn inter_vol(src: &OverlapMesh, sc: usize, tgt: &OverlapMesh, tc: usize, s: &mut TetScratch) -> f64 {
    let bb = tgt.cell_bb[tc];
    cell_cell_overlap(src, sc, tgt, tc, &bb, OverlapOp::Sum, s).0
}

/// `cellVolumeWeightMethod::findInitialSeeds`.
fn find_initial_seeds(
    src: &OverlapMesh,
    tgt: &OverlapMesh,
    src_cell_ids: &[usize],
    map_flag: &[bool],
    start_seed: usize,
    s: &mut TetScratch,
) -> Option<(usize, usize)> {
    for &src_i in src_cell_ids.iter().skip(start_seed) {
        if map_flag[src_i] {
            for pt in src.cell_points_ordered(src_i) {
                if let Some(tgt_i) = tgt.find_inside(src.points[pt]) {
                    if intersect(src, src_i, tgt, tgt_i, s) {
                        return Some((src_i, tgt_i));
                    }
                }
            }
        }
    }
    None
}

/// `cellVolumeWeightMethod::setNextCells`: the next `(src, tgt)` seed pair, or `None` when no more cells intersect.
#[allow(clippy::too_many_arguments)]
fn set_next_cells(
    src: &OverlapMesh,
    tgt: &OverlapMesh,
    start_seed: &mut usize,
    src_celli: usize,
    src_cell_ids: &[usize],
    map_flag: &[bool],
    visited: &[usize],
    seed_cells: &mut [Option<usize>],
    s: &mut TetScratch,
) -> Option<(usize, usize)> {
    // set possible seeds for later use by querying all src cell neighbours with all visited target cells
    let mut values_set: Option<(usize, usize)> = None;
    for &cell_s in &src.cell_cells[src_celli] {
        if map_flag[cell_s] && seed_cells[cell_s].is_none() {
            for &cell_t in visited {
                if intersect(src, cell_s, tgt, cell_t, s) {
                    seed_cells[cell_s] = Some(cell_t);
                    if values_set.is_none() {
                        values_set = Some((cell_s, cell_t));
                    }
                }
            }
        }
    }
    if values_set.is_some() {
        return values_set;
    }

    // try to use an existing seed
    let mut found_next_seed = false;
    for i in *start_seed..src_cell_ids.len() {
        let cell_s = src_cell_ids[i];
        if map_flag[cell_s] {
            if !found_next_seed {
                *start_seed = i;
                found_next_seed = true;
            }
            if let Some(t) = seed_cells[cell_s] {
                return Some((cell_s, t));
            }
        }
    }

    // perform a new search to find a match
    find_initial_seeds(src, tgt, src_cell_ids, map_flag, *start_seed, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_tet() -> [P; 4] {
        [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    }

    /// V&V: the tet-tet kernel against closed-form volumes. A tet against itself is its own volume `1/6`; against the same
    /// tet translated by `(1/2, 0, 0)` the common part is the corner tet `x >= 1/2` of the unit tet: edge scaled by `1/2`,
    /// volume `1/48`; disjoint tets overlap nothing.
    ///
    /// Result (2026-11-26, release): see the README "Round 28" table; the bounds here are 1e-14 relative.
    #[test]
    fn tet_tet_overlap_closed_form() {
        let a = unit_tet();
        let mut s = TetScratch::default();
        let mut v = 0.0;
        tet_tet_overlap(&a, &a, &mut v, &mut s);
        assert!((v.abs() - 1.0 / 6.0).abs() < 1.0e-14, "self overlap {v}");

        let shifted: [P; 4] = a.map(|p| [p[0] + 0.5, p[1], p[2]]);
        let mut v = 0.0;
        tet_tet_overlap(&a, &shifted, &mut v, &mut s);
        // the shifted tet is (1/2,0,0),(3/2,0,0),(1/2,1,0),(1/2,0,1); its overlap with the unit tet is the tet
        // bounded by x >= 1/2, y >= 0, z >= 0, x + y + z <= 1 and x - 1/2 + y + z <= 1: the first binds, edge 1/2
        assert!((v.abs() - 1.0 / 48.0).abs() < 1.0e-14, "half-shifted overlap {v}");

        let far: [P; 4] = a.map(|p| [p[0] + 5.0, p[1], p[2]]);
        let mut v = 0.0;
        tet_tet_overlap(&a, &far, &mut v, &mut s);
        assert_eq!(v, 0.0);
    }

    /// Every one of the six `nPos == 2` edge branches and the `nPos == 1` / `3` branches of the slicer keeps the right
    /// volume: a plane through `x = c` cuts the unit tet; the part with `x > c` has volume `(1 - c)^3 / 6` for every
    /// orientation of the tet (vertices permuted so each branch is hit).
    #[test]
    fn slicer_keeps_the_exact_volume_in_every_branch() {
        let base = unit_tet();
        let perms: [[usize; 4]; 6] = [[0, 1, 2, 3], [1, 0, 2, 3], [2, 3, 0, 1], [3, 2, 1, 0], [1, 2, 3, 0], [0, 3, 1, 2]];
        // planes that leave 1, 2 or 3 vertices above, along +x or -x: x > 0.25 (1 vertex), the plane x + y > 0.5 (2 above
        // for the permuted tets), x > -0.25 (3 above, one cut off)
        let cuts: [(P, P); 4] = [
            ([0.25, 0.0, 0.0], [1.0, 0.0, 0.0]),
            ([0.0, 0.0, 0.0], [-1.0, -1.0, 0.0]),
            ([-0.25, 0.0, 0.0], [1.0, 0.0, 0.0]),
            ([0.5, 0.0, 0.0], [0.6, 0.8, 0.0]),
        ];
        for perm in perms {
            let t = [base[perm[0]], base[perm[1]], base[perm[2]], base[perm[3]]];
            for (origin, normal) in cuts {
                let mut above = Vec::new();
                slice_above(&(origin, normal), &t, &mut above);
                let kept: f64 = above.iter().map(|x| tet_mag(x).abs()).sum();
                // the exact volume above the plane, by clipping the base tet's vertex set analytically via a fine
                // reference: sum of |tet_mag| of the pieces must also equal total minus below, tested through symmetry:
                // slicing with the reversed normal gives the complement
                let rev = (origin, [-normal[0], -normal[1], -normal[2]]);
                let mut below = Vec::new();
                slice_above(&rev, &t, &mut below);
                let kept_below: f64 = below.iter().map(|x| tet_mag(x).abs()).sum();
                assert!(
                    (kept + kept_below - 1.0 / 6.0).abs() < 1.0e-14,
                    "perm {perm:?} plane {origin:?}/{normal:?}: {kept} + {kept_below} != 1/6"
                );
            }
        }
        // the closed form for the x > c cut of the unit tet
        let mut above = Vec::new();
        slice_above(&([0.25, 0.0, 0.0], [1.0, 0.0, 0.0]), &unit_tet(), &mut above);
        let kept: f64 = above.iter().map(|x| tet_mag(x).abs()).sum();
        let c = 0.25;
        assert!((kept - (1.0 - c) * (1.0 - c) * (1.0 - c) / 6.0).abs() < 1.0e-14, "x > 0.25 keeps {kept}");
    }

    /// A mesh against itself: one overlap per cell, the cell itself, weight exactly 1, `V` the mesh volume.
    #[test]
    fn identical_meshes_map_as_the_identity() {
        let m = OverlapMesh::box_grid(3, 2, 2, [0.0; 3], [3.0, 1.0, 2.0]);
        let a = CellVolumeWeightAddressing::new_normalised(&m, &m);
        for c in 0..m.n_cells() {
            assert_eq!(a.src_to_tgt_addr[c], vec![c], "cell {c}");
            assert_eq!(a.src_to_tgt_wght[c], vec![1.0]);
            assert_eq!(a.tgt_to_src_addr[c], vec![c]);
        }
        assert!((a.v - 6.0).abs() < 1.0e-12, "V {}", a.v);
    }

    /// V&V (gh:#651 slab case 1): partial overlap. Source 10 cells on [0,1] at a uniform 900 K, target 10 cells on [0,2].
    /// Upstream's operator, with the roles of GeN-Foam's `mapTgtToSrc` (the receiving mesh is upstream's source):
    /// target cells 0-4 are covered and take 900 K, cells 5-9 are uncovered and KEEP their value. The previous port gave 450 K
    /// everywhere. Result (2026-11-26, release): 900 K on cells 0-4 to 1e-13, cells 5-9 unchanged exactly.
    #[test]
    fn slab_partial_overlap_keeps_uncovered_cells_and_does_not_rescale() {
        let provider = OverlapMesh::box_grid(10, 1, 1, [0.0; 3], [1.0, 1.0, 1.0]);
        let receiver = OverlapMesh::box_grid(10, 1, 1, [0.0; 3], [2.0, 1.0, 1.0]);
        // upstream object: src = receiver, tgt = provider
        let a = CellVolumeWeightAddressing::new_normalised(&receiver, &provider);
        let field = vec![900.0; 10];
        let mut result: Vec<f64> = (0..10).map(|i| 100.0 + i as f64).collect();
        a.map_tgt_to_src_scalar(&field, &mut result);
        for i in 0..5 {
            assert!((result[i] - 900.0).abs() < 1.0e-12, "covered cell {i}: {}", result[i]);
        }
        for i in 5..10 {
            assert_eq!(result[i], 100.0 + i as f64, "uncovered cell {i} must keep its value");
        }
    }

    /// V&V (gh:#651 slab case 2): same extent, linear field `600 + 600 x`, 10 -> 3 cells. With a piecewise-constant source
    /// the volume-weighted mean of the source CELL VALUES is 702 / 900 / 1098 (cell 0: 0.3 m of cells 0-2 at 630, 690, 750
    /// plus 1/30 m of cell 3 at 810, over 1/3 m). The issue text and README prediction P6 said 700 / 900 / 1100 (the
    /// continuous means): that was wrong for upstream's operator. Previous port: 697.8 / 879.8 / 1122.5.
    /// Result (2026-11-26, release): 702 / 900 / 1098 to 1e-10.
    #[test]
    fn slab_linear_field_ten_to_three_cells_is_exact() {
        let provider = OverlapMesh::box_grid(10, 1, 1, [0.0; 3], [1.0, 1.0, 1.0]);
        let receiver = OverlapMesh::box_grid(3, 1, 1, [0.0; 3], [1.0, 1.0, 1.0]);
        let a = CellVolumeWeightAddressing::new_normalised(&receiver, &provider);
        let field: Vec<f64> = provider.cell_centres().iter().map(|c| 600.0 + 600.0 * c.x).collect();
        let mut result = vec![0.0; 3];
        a.map_tgt_to_src_scalar(&field, &mut result);
        for (got, want) in result.iter().zip([702.0, 900.0, 1098.0]) {
            assert!((got - want).abs() < 1.0e-10, "{got} != {want}");
        }
        // the other direction (mapSrcToTgt) of the same object: 3 cells -> 10 cells, by the same weights
        let mut back = vec![0.0; 10];
        let coarse: Vec<f64> = vec![702.0, 900.0, 1098.0];
        a.map_src_to_tgt_scalar(&coarse, &mut back);
        // every fine cell lies inside one or two coarse cells: it takes their volume-weighted mean
        for (i, v) in back.iter().enumerate() {
            let x0 = i as f64 / 10.0;
            let x1 = (i as f64 + 1.0) / 10.0;
            let mut want = 0.0;
            for (j, cv) in coarse.iter().enumerate() {
                let lo = (j as f64 / 3.0).max(x0);
                let hi = ((j as f64 + 1.0) / 3.0).min(x1);
                if hi > lo {
                    want += (hi - lo) * 10.0 * cv;
                }
            }
            assert!((v - want).abs() < 1.0e-9, "fine cell {i}: {v} != {want}");
        }
    }

    /// Two different 3-D blocks over one box: the raw overlap volumes of every source cell sum to its volume, the total is
    /// the box volume, a constant maps to the same constant, and `sum V phi` of a linear field is conserved in both
    /// directions (the property the power and temperature coupling relies on). Result (2026-11-26, release): see README.
    #[test]
    fn two_blocks_of_one_box_conserve_volume_and_integrals() {
        let a_m = OverlapMesh::box_grid(4, 3, 2, [0.0; 3], [2.0, 1.5, 1.0]);
        let b_m = OverlapMesh::box_grid(3, 5, 3, [0.0; 3], [2.0, 1.5, 1.0]);
        let raw = CellVolumeWeightAddressing::calculate(&a_m, &b_m);
        assert!((raw.v - 3.0).abs() < 1.0e-12, "total overlap volume {}", raw.v);
        for c in 0..a_m.n_cells() {
            let s = sum_list(&raw.src_to_tgt_wght[c]);
            assert!((s - a_m.cell_volumes()[c]).abs() < 1.0e-12 * a_m.cell_volumes()[c], "src cell {c}: {s}");
        }
        let n = CellVolumeWeightAddressing::new_normalised(&a_m, &b_m);
        // constant
        let mut r = vec![-7.0; a_m.n_cells()];
        n.map_tgt_to_src_scalar(&vec![42.0; b_m.n_cells()], &mut r);
        assert!(r.iter().all(|x| (x - 42.0).abs() < 1.0e-12));
        // conservation of the integral of a linear field, B -> A and A -> B
        let lin = |c: &Vector3| 1.0 + 2.0 * c.x - 3.0 * c.y + 0.5 * c.z;
        let fb: Vec<f64> = b_m.cell_centres().iter().map(lin).collect();
        let mut fa = vec![0.0; a_m.n_cells()];
        n.map_tgt_to_src_scalar(&fb, &mut fa);
        let ia: f64 = fa.iter().zip(a_m.cell_volumes()).map(|(f, v)| f * v).sum();
        let ib: f64 = fb.iter().zip(b_m.cell_volumes()).map(|(f, v)| f * v).sum();
        assert!((ia - ib).abs() < 1.0e-11 * ib.abs(), "B -> A: {ia} vs {ib}");
        let mut fb2 = vec![0.0; b_m.n_cells()];
        n.map_src_to_tgt_scalar(&fa, &mut fb2);
        let ib2: f64 = fb2.iter().zip(b_m.cell_volumes()).map(|(f, v)| f * v).sum();
        assert!((ia - ib2).abs() < 1.0e-11 * ia.abs(), "A -> B: {ia} vs {ib2}");
        // the vector map is the scalar map per component
        let ub: Vec<Vector3> = b_m.cell_centres().iter().map(|c| Vector3::new(c.x, 2.0 * c.y, -c.z)).collect();
        let mut ua = vec![Vector3::new(9.0, 9.0, 9.0); a_m.n_cells()];
        n.map_tgt_to_src_vector(&ub, &mut ua);
        let mut ux = vec![9.0; a_m.n_cells()];
        n.map_tgt_to_src_scalar(&ub.iter().map(|u| u.x).collect::<Vec<_>>(), &mut ux);
        for (u, x) in ua.iter().zip(&ux) {
            assert_eq!(u.x, *x);
        }
    }

    /// A tilted pair (a unit cube against the same cube rotated about the z axis by a rational tilt): the front finds
    /// all neighbours, and the raw volumes agree with the closed-form overlap of the cube with the sheared cell, `V` below
    /// the cube volume. Also exercises `find_inside` / `point_in_cell`, a warped hexahedron and the seed search.
    #[test]
    fn sheared_cell_overlaps_by_closed_form() {
        // 1 x 1 x 1 cube against the same cube sheared in x by 0.25 y (a parallelepiped): their overlap is the cube minus
        // two triangular prisms of area 0.25 * 0.5 * 1 ... in 2-D the cube [0,1]^2 against the strip shifted at height y by
        // 0.25 y: overlap area = integral_0^1 (1 - 0.25 y) dy = 1 - 0.125 = 0.875; extruded by 1 in z: 0.875 m^3.
        let cube = OverlapMesh::box_grid(1, 1, 1, [0.0; 3], [1.0; 3]);
        let mut pts: Vec<Vector3> = Vec::new();
        for k in 0..2 {
            for j in 0..2 {
                for i in 0..2 {
                    let (x, y, z) = (i as f64, j as f64, k as f64);
                    pts.push(Vector3::new(x + 0.25 * y, y, z));
                }
            }
        }
        let pt = |i: usize, j: usize, k: usize| (k * 2 + j) * 2 + i;
        let faces = vec![
            vec![pt(0, 0, 0), pt(0, 0, 1), pt(0, 1, 1), pt(0, 1, 0)],
            vec![pt(1, 0, 0), pt(1, 1, 0), pt(1, 1, 1), pt(1, 0, 1)],
            vec![pt(0, 0, 0), pt(1, 0, 0), pt(1, 0, 1), pt(0, 0, 1)],
            vec![pt(0, 1, 0), pt(0, 1, 1), pt(1, 1, 1), pt(1, 1, 0)],
            vec![pt(0, 0, 0), pt(0, 1, 0), pt(1, 1, 0), pt(1, 0, 0)],
            vec![pt(0, 0, 1), pt(1, 0, 1), pt(1, 1, 1), pt(0, 1, 1)],
        ];
        let sheared = OverlapMesh::new(&pts, faces, vec![0; 6], vec![]).unwrap();
        let a = CellVolumeWeightAddressing::calculate(&cube, &sheared);
        assert_eq!(a.src_to_tgt_addr[0], vec![0]);
        assert!((a.v - 0.875).abs() < 1.0e-13, "sheared overlap {}", a.v);
    }
}
