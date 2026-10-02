// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! The [`UnstructuredMesh`] type: storage, accessors, derived geometry and
//! validation. Construction lives in [`super::build`].

use super::element::{ElementKind, LengthUnit, Patch, PatchKind, Zone};
use super::geometry::{
    cell_volumes_and_centres, dot, face_centre_and_area, for_each_fan_triangle, mag, tet_signed_volume,
};
use super::locator::CellLocator;

/// Relative tolerance on a cell's closure: `|sum of outward area vectors|`
/// over `sum of |area vectors|`. A closed polyhedron sums to exactly zero;
/// `1e-8` admits round-off on large meshes and rejects any missing face.
const CLOSURE_TOL: f64 = 1e-8;

/// Errors raised while building or validating an [`UnstructuredMesh`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum UnstructuredMeshError {
    /// Nothing to build.
    #[error("empty mesh: {0}")]
    Empty(&'static str),
    /// A point index past the end of the point list.
    #[error("{context}: point index {index} out of range ({n_points} points)")]
    PointOutOfRange {
        /// Where it was found.
        context: &'static str,
        /// The bad index.
        index: usize,
        /// Number of points.
        n_points: usize,
    },
    /// A cell index past the end of the cell list.
    #[error("{context}: cell index {index} out of range ({n_cells} cells)")]
    CellOutOfRange {
        /// Where it was found.
        context: &'static str,
        /// The bad index.
        index: usize,
        /// Number of cells.
        n_cells: usize,
    },
    /// A typed element with the wrong number of nodes.
    #[error("element {element}: {kind:?} needs {expected} nodes, got {got}")]
    WrongNodeCount {
        /// Element index.
        element: usize,
        /// Its kind.
        kind: ElementKind,
        /// Nodes the kind needs.
        expected: usize,
        /// Nodes given.
        got: usize,
    },
    /// An element kind that is not allowed where it was given.
    #[error("element {element}: {kind:?} is not allowed here: {reason}")]
    BadKind {
        /// Element index.
        element: usize,
        /// Its kind.
        kind: ElementKind,
        /// Why.
        reason: &'static str,
    },
    /// Elements of different dimension in one mesh.
    #[error("element {element} is {got}-D in a {expected}-D mesh")]
    MixedDimension {
        /// Element index.
        element: usize,
        /// Mesh dimension (from the first element).
        expected: usize,
        /// This element's dimension.
        got: usize,
    },
    /// A face shared by more than two cells.
    #[error("face with nodes {nodes:?} is shared by more than two cells (non-manifold)")]
    NonManifoldFace {
        /// Sorted corner nodes of the face.
        nodes: Vec<usize>,
    },
    /// A boundary facet that matches no boundary face.
    #[error("patch '{patch}': facet {nodes:?} matches no boundary face")]
    UnmatchedFacet {
        /// Patch name.
        patch: String,
        /// The facet's nodes as given.
        nodes: Vec<usize>,
    },
    /// A face listed in two patches.
    #[error("face {face} is in two patches, '{first}' and '{second}'")]
    FaceInTwoPatches {
        /// Face index.
        face: usize,
        /// First patch.
        first: String,
        /// Second patch.
        second: String,
    },
    /// A patch naming a face that does not exist.
    #[error("patch '{patch}': face {face} out of range ({n_faces} faces)")]
    FaceOutOfRange {
        /// Patch name.
        patch: String,
        /// The bad index.
        face: usize,
        /// Number of faces.
        n_faces: usize,
    },
    /// A boundary face no patch claims.
    #[error("boundary face {face} is in no patch")]
    BoundaryFaceUnpatched {
        /// Face index.
        face: usize,
    },
    /// An internal face listed in a patch.
    #[error("internal face {face} is listed in patch '{patch}'")]
    InternalFaceInPatch {
        /// Face index.
        face: usize,
        /// Patch name.
        patch: String,
    },
    /// A face with too few vertices.
    #[error("face {face} has {n} vertices; a {dim}-D mesh needs at least {min}")]
    DegenerateFace {
        /// Face index.
        face: usize,
        /// Vertices given.
        n: usize,
        /// Mesh dimension.
        dim: usize,
        /// Minimum.
        min: usize,
    },
    /// A face whose owner is also its neighbour.
    #[error("face {face}: owner and neighbour are both cell {cell}")]
    SelfNeighbour {
        /// Face index.
        face: usize,
        /// The cell.
        cell: usize,
    },
    /// A cell with too few faces to enclose anything.
    #[error("cell {cell} has {n} faces; a {dim}-D cell needs at least {min}")]
    TooFewFaces {
        /// Cell index.
        cell: usize,
        /// Faces found.
        n: usize,
        /// Mesh dimension.
        dim: usize,
        /// Minimum.
        min: usize,
    },
    /// A cell whose outward area vectors do not sum to zero.
    #[error("cell {cell} is not closed: |sum Sf| / sum |Sf| = {relative:e}")]
    OpenCell {
        /// Cell index.
        cell: usize,
        /// Relative closure residual.
        relative: f64,
    },
    /// A cell with non-positive volume, or whose tetrahedral decomposition
    /// contains an inverted tetrahedron (a concave or tangled cell).
    #[error("cell {cell}: {reason} (volume {volume:e})")]
    BadCellGeometry {
        /// Cell index.
        cell: usize,
        /// Cell volume (area in 2-D).
        volume: f64,
        /// What is wrong.
        reason: &'static str,
    },
}

/// One cell's type, nodes and faces.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CellRecord {
    pub(crate) kind: ElementKind,
    /// Nodes in the kind's local ordering (typed), or the sorted distinct
    /// vertices (polygon / polyhedron).
    pub(crate) nodes: Vec<usize>,
    /// Global face indices bounding the cell.
    pub(crate) faces: Vec<usize>,
}

/// The **neutral mesh description**: one object both a finite-volume solver
/// (polyhedral cells, owner/neighbour faces, patches) and a finite-element
/// solver (typed elements with ordered nodes) can be built from, and that a
/// Monte Carlo tally bins on (`spatial_mesh::MeshKind::Unstructured`).
///
/// See the [module docs](super) for the design and the converters.
///
/// # Invariants (checked on construction)
///
/// - Every face is wound so its area vector points **out of its owner** (into
///   its neighbour), and `owner < neighbour` for internal faces built from
///   elements (the OpenFOAM upper-triangular convention the `polyMesh`
///   converter relies on).
/// - Every boundary face is in exactly one patch; no internal face is in any.
/// - Every cell is closed and has a positive volume, and its tetrahedral
///   decomposition (see [`super::geometry`]) has no inverted tetrahedron.
///
/// Fields are private so the derived geometry cannot drift from the topology;
/// a mesh is immutable once built (share it with `Arc`).
#[derive(Debug, Clone, PartialEq)]
pub struct UnstructuredMesh {
    pub(crate) unit: LengthUnit,
    pub(crate) dim: usize,
    pub(crate) points: Vec<[f64; 3]>,
    pub(crate) faces: Vec<Vec<usize>>,
    pub(crate) owner: Vec<usize>,
    pub(crate) neighbour: Vec<Option<usize>>,
    pub(crate) cells: Vec<CellRecord>,
    pub(crate) patches: Vec<Patch>,
    pub(crate) zones: Vec<Zone>,
    // ── derived ──
    pub(crate) face_centres: Vec<[f64; 3]>,
    pub(crate) face_areas: Vec<[f64; 3]>,
    pub(crate) cell_volumes: Vec<f64>,
    pub(crate) cell_centres: Vec<[f64; 3]>,
    pub(crate) cell_bounds: Vec<[[f64; 3]; 2]>,
    pub(crate) cell_zone: Vec<Option<usize>>,
    pub(crate) bounds: [[f64; 3]; 2],
    pub(crate) locator: CellLocator,
}

impl UnstructuredMesh {
    /// Assemble from validated topology: compute the geometry, validate it
    /// and build the locator. Called by every constructor.
    pub(crate) fn finish(
        unit: LengthUnit,
        dim: usize,
        points: Vec<[f64; 3]>,
        faces: Vec<Vec<usize>>,
        owner: Vec<usize>,
        neighbour: Vec<Option<usize>>,
        cells: Vec<CellRecord>,
        patches: Vec<Patch>,
        zones: Vec<Zone>,
    ) -> Result<Self, UnstructuredMeshError> {
        let n_cells = cells.len();
        let min_face_verts = if dim == 2 { 2 } else { 3 };
        for (f, verts) in faces.iter().enumerate() {
            if verts.len() < min_face_verts {
                return Err(UnstructuredMeshError::DegenerateFace { face: f, n: verts.len(), dim, min: min_face_verts });
            }
        }
        let mut face_centres = Vec::with_capacity(faces.len());
        let mut face_areas = Vec::with_capacity(faces.len());
        for verts in &faces {
            let (c, a) = face_centre_and_area(&points, verts, dim);
            face_centres.push(c);
            face_areas.push(a);
        }
        let (cell_volumes, cell_centres) =
            cell_volumes_and_centres(n_cells, &face_centres, &face_areas, &owner, &neighbour, dim);

        // Patch membership: every boundary face exactly once, no internal face.
        let mut patch_of = vec![usize::MAX; faces.len()];
        for (p, patch) in patches.iter().enumerate() {
            for &f in &patch.faces {
                if f >= faces.len() {
                    return Err(UnstructuredMeshError::FaceOutOfRange { patch: patch.name.clone(), face: f, n_faces: faces.len() });
                }
                if neighbour[f].is_some() {
                    return Err(UnstructuredMeshError::InternalFaceInPatch { face: f, patch: patch.name.clone() });
                }
                if patch_of[f] != usize::MAX {
                    return Err(UnstructuredMeshError::FaceInTwoPatches {
                        face: f,
                        first: patches[patch_of[f]].name.clone(),
                        second: patch.name.clone(),
                    });
                }
                patch_of[f] = p;
            }
        }
        for f in 0..faces.len() {
            if neighbour[f].is_none() && patch_of[f] == usize::MAX {
                return Err(UnstructuredMeshError::BoundaryFaceUnpatched { face: f });
            }
        }

        let mut cell_zone = vec![None; n_cells];
        for (z, zone) in zones.iter().enumerate() {
            for &c in &zone.cells {
                if c >= n_cells {
                    return Err(UnstructuredMeshError::CellOutOfRange { context: "zone", index: c, n_cells });
                }
                if cell_zone[c].is_none() {
                    cell_zone[c] = Some(z);
                }
            }
        }

        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        let mut cell_bounds = Vec::with_capacity(n_cells);
        for cell in &cells {
            let mut clo = [f64::INFINITY; 3];
            let mut chi = [f64::NEG_INFINITY; 3];
            for &f in &cell.faces {
                for &v in &faces[f] {
                    for a in 0..3 {
                        clo[a] = clo[a].min(points[v][a]);
                        chi[a] = chi[a].max(points[v][a]);
                    }
                }
            }
            for a in 0..3 {
                lo[a] = lo[a].min(clo[a]);
                hi[a] = hi[a].max(chi[a]);
            }
            cell_bounds.push([clo, chi]);
        }
        let locator = CellLocator::build(lo, hi, &cell_bounds);

        let mesh = Self {
            unit,
            dim,
            points,
            faces,
            owner,
            neighbour,
            cells,
            patches,
            zones,
            face_centres,
            face_areas,
            cell_volumes,
            cell_centres,
            cell_bounds,
            cell_zone,
            bounds: [lo, hi],
            locator,
        };
        mesh.validate_cells()?;
        Ok(mesh)
    }

    /// Closure, positive volume and an uninverted decomposition, per cell.
    fn validate_cells(&self) -> Result<(), UnstructuredMeshError> {
        let min_faces = if self.dim == 2 { 3 } else { 4 };
        for c in 0..self.n_cells() {
            let faces = &self.cells[c].faces;
            if faces.len() < min_faces {
                return Err(UnstructuredMeshError::TooFewFaces { cell: c, n: faces.len(), dim: self.dim, min: min_faces });
            }
            let mut sum = [0.0; 3];
            let mut norm = 0.0;
            for &f in faces {
                let s = if self.owner[f] == c { 1.0 } else { -1.0 };
                let a = self.face_areas[f];
                sum = [sum[0] + s * a[0], sum[1] + s * a[1], sum[2] + s * a[2]];
                norm += mag(a);
            }
            let relative = if norm > 0.0 { mag(sum) / norm } else { f64::INFINITY };
            if !(relative <= CLOSURE_TOL) {
                return Err(UnstructuredMeshError::OpenCell { cell: c, relative });
            }
            let volume = self.cell_volumes[c];
            if !(volume > 0.0) {
                return Err(UnstructuredMeshError::BadCellGeometry { cell: c, volume, reason: "non-positive volume" });
            }
            let mut inverted = false;
            self.for_each_cell_simplex(c, |s| {
                let v = if self.dim == 2 {
                    let (a, b, d) = (s[0], s[1], s[2]);
                    0.5 * ((b[0] - a[0]) * (d[1] - a[1]) - (b[1] - a[1]) * (d[0] - a[0]))
                } else {
                    tet_signed_volume(s[0], s[1], s[2], s[3])
                };
                // A zero-volume simplex is legitimate (a planar face seen
                // edge-on from the centroid cannot happen in a valid cell, but a
                // collinear fan triangle of a polygon face can).
                if v < -1e-12 * volume {
                    inverted = true;
                }
            });
            if inverted {
                return Err(UnstructuredMeshError::BadCellGeometry {
                    cell: c,
                    volume,
                    reason: "inverted tetrahedron in the centroid decomposition (concave or tangled cell)",
                });
            }
        }
        Ok(())
    }

    // ── Accessors ──────────────────────────────────────────────────────────

    /// Unit of every stored coordinate, length, area and volume.
    #[inline]
    pub fn unit(&self) -> LengthUnit {
        self.unit
    }
    /// Spatial dimension, 2 or 3.
    #[inline]
    pub fn dim(&self) -> usize {
        self.dim
    }
    /// Points (vertices), in [`Self::unit`].
    #[inline]
    pub fn points(&self) -> &[[f64; 3]] {
        &self.points
    }
    /// Number of cells.
    #[inline]
    pub fn n_cells(&self) -> usize {
        self.cells.len()
    }
    /// Number of faces (internal + boundary).
    #[inline]
    pub fn n_faces(&self) -> usize {
        self.faces.len()
    }
    /// Number of internal faces.
    pub fn n_internal_faces(&self) -> usize {
        self.neighbour.iter().filter(|n| n.is_some()).count()
    }
    /// Vertex loop of face `f`, wound out of its owner.
    #[inline]
    pub fn face(&self, f: usize) -> &[usize] {
        &self.faces[f]
    }
    /// Owner cell of face `f`.
    #[inline]
    pub fn owner(&self, f: usize) -> usize {
        self.owner[f]
    }
    /// Neighbour cell of face `f`, `None` on the boundary.
    #[inline]
    pub fn neighbour(&self, f: usize) -> Option<usize> {
        self.neighbour[f]
    }
    /// Type of cell `c`.
    #[inline]
    pub fn cell_kind(&self, c: usize) -> ElementKind {
        self.cells[c].kind
    }
    /// Nodes of cell `c`: the local FE ordering for a typed cell, the sorted
    /// distinct vertices for a polygon / polyhedron.
    #[inline]
    pub fn cell_nodes(&self, c: usize) -> &[usize] {
        &self.cells[c].nodes
    }
    /// Faces bounding cell `c`.
    #[inline]
    pub fn cell_faces(&self, c: usize) -> &[usize] {
        &self.cells[c].faces
    }
    /// Boundary patches.
    #[inline]
    pub fn patches(&self) -> &[Patch] {
        &self.patches
    }
    /// Cell zones.
    #[inline]
    pub fn zones(&self) -> &[Zone] {
        &self.zones
    }
    /// The first zone cell `c` belongs to, if any.
    #[inline]
    pub fn zone_of(&self, c: usize) -> Option<usize> {
        self.cell_zone[c]
    }
    /// Centre of face `f`.
    #[inline]
    pub fn face_centre(&self, f: usize) -> [f64; 3] {
        self.face_centres[f]
    }
    /// Area vector of face `f` (out of the owner; per unit depth in 2-D).
    #[inline]
    pub fn face_area_vector(&self, f: usize) -> [f64; 3] {
        self.face_areas[f]
    }
    /// Volume of cell `c` in `unit^3` (area in `unit^2` for a 2-D mesh).
    #[inline]
    pub fn cell_volume(&self, c: usize) -> f64 {
        self.cell_volumes[c]
    }
    /// Volume-weighted centroid of cell `c`.
    #[inline]
    pub fn cell_centre(&self, c: usize) -> [f64; 3] {
        self.cell_centres[c]
    }
    /// Axis-aligned bounds `[lower, upper]` of cell `c`.
    #[inline]
    pub fn cell_bounds(&self, c: usize) -> [[f64; 3]; 2] {
        self.cell_bounds[c]
    }
    /// Axis-aligned bounds `[lower, upper]` of the whole mesh.
    #[inline]
    pub fn bounds(&self) -> [[f64; 3]; 2] {
        self.bounds
    }
    /// The spatial index over cell bounding boxes.
    #[inline]
    pub fn locator(&self) -> &CellLocator {
        &self.locator
    }
    /// Sum of cell volumes, in `unit^3`.
    pub fn total_volume(&self) -> f64 {
        self.cell_volumes.iter().sum()
    }
    /// Index of the patch named `name`.
    pub fn patch_index(&self, name: &str) -> Option<usize> {
        self.patches.iter().position(|p| p.name == name)
    }

    /// Visit every simplex of cell `c`'s centroid decomposition (see
    /// [`super::geometry`]): in 3-D the tetrahedra
    /// `[face centre, v_i+1, v_i, cell centre]` (owner side) or
    /// `[face centre, v_i, v_i+1, cell centre]` (neighbour side), both
    /// positively oriented for a valid cell; in 2-D the triangles
    /// `[v_a, v_b, cell centre]` per edge (the 4th entry repeats the centre).
    ///
    /// The tetrahedra of all cells tile the mesh; neighbouring cells share
    /// each face's fan triangles exactly.
    #[inline]
    pub fn for_each_cell_simplex<F: FnMut([[f64; 3]; 4])>(&self, c: usize, mut f: F) {
        let cc = self.cell_centres[c];
        for &face in &self.cells[c].faces {
            let owner_side = self.owner[face] == c;
            if self.dim == 2 {
                let (a, b) = (self.points[self.faces[face][0]], self.points[self.faces[face][1]]);
                // The edge a -> b is counter-clockwise for its owner.
                if owner_side {
                    f([a, b, cc, cc]);
                } else {
                    f([b, a, cc, cc]);
                }
                continue;
            }
            for_each_fan_triangle(&self.points, &self.faces[face], self.face_centres[face], |fc, p, q| {
                if owner_side {
                    f([fc, q, p, cc]);
                } else {
                    f([fc, p, q, cc]);
                }
            });
        }
    }

    /// A copy with every coordinate rescaled into `unit`.
    pub fn with_unit(&self, unit: LengthUnit) -> Self {
        if unit == self.unit {
            return self.clone();
        }
        let s = self.unit.cm_per_unit() / unit.cm_per_unit();
        let sc = |p: [f64; 3]| [p[0] * s, p[1] * s, p[2] * s];
        let area_scale = if self.dim == 2 { s } else { s * s };
        let vol_scale = if self.dim == 2 { s * s } else { s * s * s };
        let mut m = self.clone();
        m.unit = unit;
        m.points.iter_mut().for_each(|p| *p = sc(*p));
        m.face_centres.iter_mut().for_each(|p| *p = sc(*p));
        m.face_areas.iter_mut().for_each(|a| *a = [a[0] * area_scale, a[1] * area_scale, a[2] * area_scale]);
        m.cell_volumes.iter_mut().for_each(|v| *v *= vol_scale);
        m.cell_centres.iter_mut().for_each(|p| *p = sc(*p));
        m.cell_bounds.iter_mut().for_each(|b| *b = [sc(b[0]), sc(b[1])]);
        m.bounds = [sc(self.bounds[0]), sc(self.bounds[1])];
        m.locator = CellLocator::build(m.bounds[0], m.bounds[1], &m.cell_bounds);
        m
    }

    /// Move every boundary face whose centre and outward unit normal satisfy
    /// `select` into a new patch `name` of type `kind`, taking it out of
    /// whichever patch held it. Patches left empty are dropped.
    ///
    /// The usual way to name the faces of a generated mesh
    /// (`|c, n| n[0] < -0.99` for the `x`-min face, say).
    pub fn with_patch_where<F: Fn([f64; 3], [f64; 3]) -> bool>(
        mut self,
        name: impl Into<String>,
        kind: PatchKind,
        select: F,
    ) -> Self {
        let mut taken = Vec::new();
        for p in &mut self.patches {
            p.faces.retain(|&f| {
                let a = self.face_areas[f];
                let m = mag(a);
                let n = if m > 0.0 { [a[0] / m, a[1] / m, a[2] / m] } else { a };
                if select(self.face_centres[f], n) {
                    taken.push(f);
                    false
                } else {
                    true
                }
            });
        }
        self.patches.retain(|p| !p.faces.is_empty());
        if !taken.is_empty() {
            taken.sort_unstable();
            self.patches.push(Patch { name: name.into(), kind, faces: taken });
        }
        self
    }

    /// Replace the cell zones.
    ///
    /// # Errors
    /// A zone naming a cell that does not exist.
    pub fn with_zones(mut self, zones: Vec<Zone>) -> Result<Self, UnstructuredMeshError> {
        let n = self.n_cells();
        let mut cell_zone = vec![None; n];
        for (z, zone) in zones.iter().enumerate() {
            for &c in &zone.cells {
                if c >= n {
                    return Err(UnstructuredMeshError::CellOutOfRange { context: "zone", index: c, n_cells: n });
                }
                if cell_zone[c].is_none() {
                    cell_zone[c] = Some(z);
                }
            }
        }
        self.zones = zones;
        self.cell_zone = cell_zone;
        Ok(self)
    }

    /// Outward normal component check used by the converters: `true` if the
    /// area vector of face `f` points from `owner` toward `neighbour`
    /// (positive projection on the centre-to-centre vector).
    pub fn face_is_oriented(&self, f: usize) -> bool {
        let o = self.cell_centres[self.owner[f]];
        let d = match self.neighbour[f] {
            Some(n) => {
                let c = self.cell_centres[n];
                [c[0] - o[0], c[1] - o[1], c[2] - o[2]]
            }
            None => {
                let c = self.face_centres[f];
                [c[0] - o[0], c[1] - o[1], c[2] - o[2]]
            }
        };
        dot(self.face_areas[f], d) > 0.0
    }
}
