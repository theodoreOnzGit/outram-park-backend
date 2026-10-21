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

//! The two constructors of [`UnstructuredMesh`]: from typed **elements** (the
//! finite-element view) and from **polyhedral faces** (the `polyMesh` view).
//! Each derives what the other view needs.

use std::collections::HashMap;

use super::element::{BoundarySpec, Element, ElementKind, LengthUnit, Patch, PatchKind, PolyFace, Zone};
use super::mesh::{CellRecord, UnstructuredMesh, UnstructuredMeshError};

/// Name of the patch that collects boundary faces no
/// [`BoundarySpec`] claimed — OpenFOAM `blockMesh`'s default patch name.
///
/// `blockMesh` gives `defaultFaces` the type `empty`; this crate gives it
/// [`PatchKind::Patch`] instead, deliberately: `empty` means "2-D reduced
/// direction" to an FV solver, which would be wrong for every 3-D mesh built
/// here. Rename or retype it with
/// [`UnstructuredMesh::with_patch_where`].
pub const DEFAULT_PATCH: &str = "defaultFaces";

impl UnstructuredMesh {
    /// Build from **typed elements** (the finite-element view).
    ///
    /// Faces are derived by matching element faces on their sorted corner
    /// nodes: the first element to produce a face owns it (so the face keeps
    /// that element's outward winding) and the second becomes its neighbour.
    /// Because elements are visited in index order, `owner < neighbour` holds
    /// for every internal face. A face met a third time is non-manifold and an
    /// error.
    ///
    /// Boundary faces are assigned to the patches of `boundaries` by matching
    /// each facet's node **set**; any boundary face left over goes to
    /// [`DEFAULT_PATCH`].
    ///
    /// # Arguments
    /// - `unit` — the unit `points` are in.
    /// - `points` — node coordinates; a 2-D mesh lies in `z = 0`.
    /// - `elements` — typed elements, all of the same dimension.
    /// - `boundaries` — named boundary facet sets (may be empty).
    /// - `zones` — named cell sets (may be empty).
    ///
    /// # Errors
    /// Empty input, an untyped kind, mixed dimensions, a wrong node count, an
    /// out-of-range node, a non-manifold face, an unmatched facet, a face in
    /// two patches, or a cell that fails the geometric checks of
    /// [`UnstructuredMesh`] (inverted, open, concave).
    pub fn from_elements(
        unit: LengthUnit,
        points: Vec<[f64; 3]>,
        elements: Vec<Element>,
        boundaries: Vec<BoundarySpec>,
        zones: Vec<Zone>,
    ) -> Result<Self, UnstructuredMeshError> {
        if elements.is_empty() {
            return Err(UnstructuredMeshError::Empty("from_elements: no elements"));
        }
        let dim = elements[0].kind.dim();
        let n_points = points.len();
        let mut faces: Vec<Vec<usize>> = Vec::new();
        let mut owner: Vec<usize> = Vec::new();
        let mut neighbour: Vec<Option<usize>> = Vec::new();
        let mut cells: Vec<CellRecord> = Vec::with_capacity(elements.len());
        // Keyed on sorted corner nodes; only inserted into and looked up, never
        // iterated, so face numbering is deterministic (insertion order).
        let mut by_key: HashMap<Vec<usize>, usize> = HashMap::new();

        for (e, el) in elements.into_iter().enumerate() {
            let Some(nn) = el.kind.n_nodes() else {
                return Err(UnstructuredMeshError::BadKind {
                    element: e,
                    kind: el.kind,
                    reason: "from_elements takes typed elements; use from_polyhedral for polygons/polyhedra",
                });
            };
            if el.kind.dim() != dim {
                return Err(UnstructuredMeshError::MixedDimension { element: e, expected: dim, got: el.kind.dim() });
            }
            if el.nodes.len() != nn {
                return Err(UnstructuredMeshError::WrongNodeCount {
                    element: e,
                    kind: el.kind,
                    expected: nn,
                    got: el.nodes.len(),
                });
            }
            if let Some(&bad) = el.nodes.iter().find(|&&n| n >= n_points) {
                return Err(UnstructuredMeshError::PointOutOfRange { context: "element node", index: bad, n_points });
            }
            let mut cell_faces = Vec::with_capacity(el.kind.local_faces().len());
            for lf in el.kind.local_faces() {
                let verts: Vec<usize> = lf.iter().map(|&i| el.nodes[i]).collect();
                let mut key = verts.clone();
                key.sort_unstable();
                match by_key.get(&key) {
                    None => {
                        let f = faces.len();
                        by_key.insert(key, f);
                        faces.push(verts);
                        owner.push(e);
                        neighbour.push(None);
                        cell_faces.push(f);
                    }
                    Some(&f) => {
                        if neighbour[f].is_some() || owner[f] == e {
                            return Err(UnstructuredMeshError::NonManifoldFace { nodes: key });
                        }
                        neighbour[f] = Some(e);
                        cell_faces.push(f);
                    }
                }
            }
            cells.push(CellRecord { kind: el.kind, nodes: el.nodes, faces: cell_faces });
        }

        let mut patch_of = vec![usize::MAX; faces.len()];
        let mut patches = Vec::with_capacity(boundaries.len() + 1);
        for spec in boundaries {
            let mut pf = Vec::with_capacity(spec.facets.len());
            for facet in &spec.facets {
                let mut key = facet.clone();
                key.sort_unstable();
                let f = match by_key.get(&key) {
                    Some(&f) if neighbour[f].is_none() => f,
                    _ => {
                        return Err(UnstructuredMeshError::UnmatchedFacet {
                            patch: spec.name.clone(),
                            nodes: facet.clone(),
                        })
                    }
                };
                if patch_of[f] != usize::MAX {
                    let first: &Patch = &patches[patch_of[f]];
                    return Err(UnstructuredMeshError::FaceInTwoPatches {
                        face: f,
                        first: first.name.clone(),
                        second: spec.name.clone(),
                    });
                }
                patch_of[f] = patches.len();
                pf.push(f);
            }
            pf.sort_unstable();
            patches.push(Patch { name: spec.name, kind: spec.kind, faces: pf });
        }
        let rest: Vec<usize> =
            (0..faces.len()).filter(|&f| neighbour[f].is_none() && patch_of[f] == usize::MAX).collect();
        if !rest.is_empty() {
            patches.push(Patch { name: DEFAULT_PATCH.to_string(), kind: PatchKind::Patch, faces: rest });
        }
        patches.retain(|p| !p.faces.is_empty());

        Self::finish(unit, dim, points, faces, owner, neighbour, cells, patches, zones)
    }

    /// Build from **polyhedral faces** (the OpenFOAM `polyMesh` view; 3-D).
    ///
    /// Each face is wound out of its owner. Cells are recovered from the
    /// owner/neighbour lists, and each is **recognised** as a
    /// [`ElementKind::Tet4`] or [`ElementKind::Hex8`] when its faces make one
    /// (with its nodes put in the FE ordering), otherwise it stays a
    /// [`ElementKind::Polyhedron`]. That recognition is what lets a `blockMesh`
    /// hex mesh go on to a finite-element solver.
    ///
    /// # Errors
    /// Empty input, an out-of-range point or cell, a face that is its own
    /// neighbour, patch-membership errors, or a cell that fails the geometric
    /// checks of [`UnstructuredMesh`].
    pub fn from_polyhedral(
        unit: LengthUnit,
        points: Vec<[f64; 3]>,
        faces: Vec<PolyFace>,
        n_cells: usize,
        patches: Vec<Patch>,
        zones: Vec<Zone>,
    ) -> Result<Self, UnstructuredMeshError> {
        if n_cells == 0 || faces.is_empty() {
            return Err(UnstructuredMeshError::Empty("from_polyhedral: no cells or no faces"));
        }
        let n_points = points.len();
        let mut verts_list = Vec::with_capacity(faces.len());
        let mut owner = Vec::with_capacity(faces.len());
        let mut neighbour = Vec::with_capacity(faces.len());
        let mut cell_faces: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
        for (f, face) in faces.into_iter().enumerate() {
            if let Some(&bad) = face.verts.iter().find(|&&v| v >= n_points) {
                return Err(UnstructuredMeshError::PointOutOfRange { context: "face vertex", index: bad, n_points });
            }
            for c in std::iter::once(face.owner).chain(face.neighbour) {
                if c >= n_cells {
                    return Err(UnstructuredMeshError::CellOutOfRange { context: "face owner/neighbour", index: c, n_cells });
                }
            }
            if face.neighbour == Some(face.owner) {
                return Err(UnstructuredMeshError::SelfNeighbour { face: f, cell: face.owner });
            }
            cell_faces[face.owner].push(f);
            if let Some(n) = face.neighbour {
                cell_faces[n].push(f);
            }
            verts_list.push(face.verts);
            owner.push(face.owner);
            neighbour.push(face.neighbour);
        }
        let cells = cell_faces
            .into_iter()
            .enumerate()
            .map(|(c, fs)| {
                let (kind, nodes) = recognise(c, &fs, &verts_list, &owner);
                CellRecord { kind, nodes, faces: fs }
            })
            .collect();
        Self::finish(unit, 3, points, verts_list, owner, neighbour, cells, patches, zones)
    }
}

/// The vertex loop of face `f` wound **outward from cell `c`**.
fn outward(c: usize, f: usize, faces: &[Vec<usize>], owner: &[usize]) -> Vec<usize> {
    let mut v = faces[f].clone();
    if owner[f] != c {
        v.reverse();
    }
    v
}

/// Recognise a polyhedral cell as a Tet4 or Hex8 and order its nodes; fall
/// back to a Polyhedron with its sorted distinct vertices.
fn recognise(c: usize, fs: &[usize], faces: &[Vec<usize>], owner: &[usize]) -> (ElementKind, Vec<usize>) {
    let mut distinct: Vec<usize> = fs.iter().flat_map(|&f| faces[f].iter().copied()).collect();
    distinct.sort_unstable();
    distinct.dedup();
    let poly = (ElementKind::Polyhedron, distinct.clone());

    if fs.len() == 4 && distinct.len() == 4 && fs.iter().all(|&f| faces[f].len() == 3) {
        // Outward [a, b, c] is the Tet4 local face [0, 2, 1].
        let o = outward(c, fs[0], faces, owner);
        let Some(&d) = distinct.iter().find(|v| !o.contains(v)) else { return poly };
        return (ElementKind::Tet4, vec![o[0], o[2], o[1], d]);
    }
    if fs.len() == 6 && distinct.len() == 8 && fs.iter().all(|&f| faces[f].len() == 4) {
        // Outward [p, q, r, s] is the Hex8 local face [0, 3, 2, 1].
        let o = outward(c, fs[0], faces, owner);
        let bottom = [o[0], o[3], o[2], o[1]];
        let mut nodes = bottom.to_vec();
        for &b in &bottom {
            // The top partner of b: its one edge-neighbour off the bottom face.
            let mut partner = None;
            for &f in fs {
                let lp = &faces[f];
                let n = lp.len();
                for i in 0..n {
                    let (u, v) = (lp[i], lp[(i + 1) % n]);
                    let other = if u == b { v } else if v == b { u } else { continue };
                    if !bottom.contains(&other) {
                        match partner {
                            None => partner = Some(other),
                            Some(p) if p == other => {}
                            Some(_) => return poly,
                        }
                    }
                }
            }
            match partner {
                Some(p) => nodes.push(p),
                None => return poly,
            }
        }
        let mut check = nodes.clone();
        check.sort_unstable();
        check.dedup();
        if check.len() != 8 {
            return poly;
        }
        // Every face must be one of the six Hex8 local faces (as node sets).
        let mut expected: Vec<Vec<usize>> = ElementKind::Hex8
            .local_faces()
            .iter()
            .map(|lf| {
                let mut k: Vec<usize> = lf.iter().map(|&i| nodes[i]).collect();
                k.sort_unstable();
                k
            })
            .collect();
        expected.sort();
        let mut actual: Vec<Vec<usize>> = fs
            .iter()
            .map(|&f| {
                let mut k = faces[f].clone();
                k.sort_unstable();
                k
            })
            .collect();
        actual.sort();
        if expected == actual {
            return (ElementKind::Hex8, nodes);
        }
    }
    poly
}
