// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh v1.1 (github.com/DAFoam/cfmesh, commit e944f6ba695f),
// meshLibrary/utilities/:
//   decomposeCells/decomposeCells.{C,H}, decomposeCellsDecomposition.C,
//     decomposeCellsPyramids.C          (decomposeMesh, checkFaceConnections,
//                                         findTopVertex,
//                                         decomposeCellIntoPyramids)
//   faceDecomposition/decomposeFaces.{C,H}  (decomposeMeshFaces,
//                                         decomposeConcaveInternalFaces)
//   faceDecomposition/faceDecomposition.{C,H} (concaveVertex, isFaceConvex,
//                                         isFacePlanar, decomposeFace,
//                                         decomposeFaceIntoTriangles)
//   helperFunctions/helperFunctionsTopologyManipulationI.H (mergeTwoFaces,
//                                         sortEdgeChain, removeEdgesFromFace,
//                                         createFaceFromRemovedPart,
//                                         shareAnEdge)
//   surfaceTools/surfaceMorpherCells/surfaceMorpherCells.{C,H},
//     surfaceMorpherCellsCreateBoundaryFaces.C,
//     surfaceMorpherCellsMorphInternalFaces.C
//   smoothers/topology/topologicalCleaner/topologicalCleaner.{C,H},
//     topologicalCleanerInvalidVertices.C,
//     topologyCleanerNonConsecutiveBoundaryVertices.C,
//     topologyCleanerNonMappableCells.C
//   smoothers/topology/checkCellConnectionsOverFaces/
//   smoothers/topology/checkIrregularSurfaceConnections/
//   smoothers/topology/checkBoundaryFacesSharingTwoEdges/
//   smoothers/topology/checkNonMappableCellConnections/
//   meshes/polyMeshGenModifier/ (removeCells, removeFaces, replaceBoundary,
//                                removeUnusedVertices: the semantics, not the
//                                face swaps)
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

//! **Topology repair and decomposition** (gh:#1267, gh:#1268): cfMesh's
//! cell and face decomposition, the `surfaceMorpherCells` and
//! `topologicalCleaner` stages of `cartesianMesh`'s surface preparation, and
//! the four connectivity checks `cartesianMesh` runs on the background mesh.
//!
//! # The working mesh
//!
//! Upstream edits a `polyMeshGen` in place. [`TopoMesh`] is its face-based
//! analogue: points, faces with owner, neighbour and (boundary) patch, and
//! the patch names. Cells are implicit, their faces listed in ascending face
//! order ([`TopoMesh::cell_faces`]), which is the order OpenFOAM builds
//! `cells()` from `owner`/`neighbour`.
//!
//! **Face and point numbering differ from upstream; the geometry and
//! topology do not.** Upstream's `polyMeshGenModifier::removeCells` and
//! `reorderBoundaryFaces` swap faces to keep internal faces first; here
//! faces keep their relative order and [`TopoMesh::into_volume_mesh`] sorts
//! them (internal first, then each patch). [`decompose_cells`] numbers its fan
//! points in the order it finds the faces (the order `correct_edges` always
//! used, pinned by `correct_edges::tests::correct_edges_is_pinned_bit_for_bit`),
//! where upstream's `decomposeMeshFaces` numbers them in face order. MPI
//! branches are not ported (serial only; `CLAUDE.md` of this crate).
//!
//! # Upstream behaviour kept, and said so
//!
//! - `removeCells` puts every boundary face into **one** patch,
//!   `defaultFaces`, when the mesh has other than exactly one patch
//!   (`reorderBoundaryFaces`). Upstream runs these stages before patches
//!   exist; [`TopoMesh::remove_cells`] does the same and says so in its
//!   return value.
//! - `topologicalCleaner::checkInvalidConnectionsForVerticesCells` appends
//!   one copy of the point per cell group and relabels group `m` to
//!   `nPoints + m − 1`, so the first copy is never used (removed by
//!   `removeUnusedVertices`). Harmless; ported as is.
//! - **Upstream defect, ported as is:**
//!   `checkBoundaryFacesSharingTwoEdges::improveTopology` writes
//!   `decomposeCell[owner[faceI]];` with no assignment, so no cell is ever
//!   marked and only the faces are fanned. [`BoundaryFacesSharingTwoEdges::improve_topology`]
//!   does the same (its test pins it); fixing it would change upstream's
//!   output.

use crate::correct_edges::foam_face_centre;
use crate::math::Vec3;
use crate::surface_engine::{ring_area_vector, SurfaceEngine, SurfacePartitioner, NOT_BOUNDARY};
use crate::volume_mesh::{BoundaryPatch, VolumeMesh};
use std::collections::{BTreeMap, BTreeSet};

/// OpenFOAM `SMALL` for `double`.
const SMALL: f64 = 1.0e-15;

/// A topology stage that upstream would stop with `FatalError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopologyError {
    /// `faceDecomposition::concaveVertex`: "Face has more than one concave
    /// vertex. Cannot continue".
    MoreThanOneConcaveVertex,
    /// `help::sortEdgeChain`: "Chain is not oriented correctly!".
    ChainNotOriented,
    /// `help::sortEdgeChain` on edges that do not form one closed chain
    /// (upstream returns a list with unset trailing labels; here it is an
    /// error).
    OpenChain,
    /// `surfaceMorpherCells::morphMesh` did not settle in the iteration
    /// guard (upstream has no guard).
    NoConvergence,
}

/// cfMesh's `polyMeshGen` as a face list (see the module documentation).
#[derive(Debug, Clone)]
pub struct TopoMesh {
    /// Point positions.
    pub points: Vec<Vec3>,
    /// Face rings, wound out of the owner.
    pub faces: Vec<Vec<usize>>,
    /// Owner cell of each face.
    pub owner: Vec<usize>,
    /// Neighbour cell of each face, `None` on the boundary.
    pub neighbour: Vec<Option<usize>>,
    /// Patch of each boundary face (`None` for internal faces, or a boundary
    /// face in no patch).
    pub region: Vec<Option<usize>>,
    /// Number of cells.
    pub n_cells: usize,
    /// Patch names, indexed by [`TopoMesh::region`].
    pub patch_names: Vec<String>,
    /// The cells' face lists of the [`VolumeMesh`] this came from (gh:#1385),
    /// in this mesh's face labels. Carried back out by
    /// [`TopoMesh::into_volume_mesh`] only while they are still consistent
    /// with `owner` / `neighbour` / `n_cells`, so an operation that changes
    /// the cells or their faces drops them without being told to.
    pub cells: Option<Vec<Vec<usize>>>,
}

/// An undirected edge equal to `(a, b)` (OpenFOAM `edge::operator==`).
fn same_edge(e: (usize, usize), a: usize, b: usize) -> bool {
    (e.0 == a && e.1 == b) || (e.0 == b && e.1 == a)
}

/// OpenFOAM's `face::reverseFace`: the first point stays, the rest reverse
/// (gh:#1385; a plain reversal is the same ring rotated, which changes the
/// written file).
pub fn reverse_face(ring: &[usize]) -> Vec<usize> {
    let mut r = ring.to_vec();
    r[1..].reverse();
    r
}

/// The edges of `ring`, edge `i` from point `i` to `i + 1` (`face::edges`).
fn ring_edges(ring: &[usize]) -> Vec<(usize, usize)> {
    let k = ring.len();
    (0..k).map(|i| (ring[i], ring[(i + 1) % k])).collect()
}

impl TopoMesh {
    /// The working form of `mesh`: each boundary face in the patch whose
    /// range holds it.
    pub fn from_volume_mesh(mesh: &VolumeMesh) -> Self {
        let mut region = vec![None; mesh.face_count()];
        for (pi, p) in mesh.patches.iter().enumerate() {
            for r in region.iter_mut().skip(p.start_face).take(p.n_faces) {
                *r = Some(pi);
            }
        }
        TopoMesh {
            points: mesh.points.clone(),
            faces: mesh.faces.clone(),
            owner: mesh.owner.clone(),
            neighbour: mesh.neighbour.clone(),
            region,
            n_cells: mesh.n_cells,
            patch_names: mesh.patches.iter().map(|p| p.name.clone()).collect(),
            cells: if mesh.carried_cells_are_consistent() { mesh.cells.clone() } else { None },
        }
    }

    /// Back to a [`VolumeMesh`] with its per-face patch: internal faces
    /// first, then each named patch contiguous, then boundary faces in no
    /// named patch as `unassigned` (the order `correct_edges` always wrote).
    pub fn into_volume_mesh(self) -> (VolumeMesh, Vec<Option<usize>>) {
        let w = self;
        let names = &w.patch_names;
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
            faces: order.iter().map(|&f| w.faces[f].clone()).collect(),
            owner: order.iter().map(|&f| w.owner[f]).collect(),
            neighbour: order.iter().map(|&f| w.neighbour[f]).collect(),
            n_cells: w.n_cells,
            patches,
            points: w.points,
            cells,
        };
        let regions = order.iter().map(|&f| w.region[f]).collect();
        (out, regions)
    }

    /// Faces of each cell, ascending.
    pub fn cell_faces(&self) -> Vec<Vec<usize>> {
        let mut cf: Vec<Vec<usize>> = vec![Vec::new(); self.n_cells];
        for f in 0..self.faces.len() {
            cf[self.owner[f]].push(f);
            if let Some(n) = self.neighbour[f] {
                cf[n].push(f);
            }
        }
        cf
    }

    /// Cells at each point, ascending (`polyMeshGenAddressing::pointCells`).
    pub fn point_cells(&self) -> Vec<Vec<usize>> {
        let mut pc: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); self.points.len()];
        for f in 0..self.faces.len() {
            for &v in &self.faces[f] {
                pc[v].insert(self.owner[f]);
                if let Some(n) = self.neighbour[f] {
                    pc[v].insert(n);
                }
            }
        }
        pc.into_iter().map(|s| s.into_iter().collect()).collect()
    }

    /// Cells across the faces of each cell, in its face order, each once
    /// (`polyMeshGenAddressing::cellCells`).
    pub fn cell_cells(&self) -> Vec<Vec<usize>> {
        self.cell_faces()
            .iter()
            .enumerate()
            .map(|(c, fs)| {
                let mut out: Vec<usize> = Vec::new();
                for &f in fs {
                    let o = if self.owner[f] == c { self.neighbour[f] } else { Some(self.owner[f]) };
                    if let Some(o) = o {
                        if !out.contains(&o) {
                            out.push(o);
                        }
                    }
                }
                out
            })
            .collect()
    }

    /// Replace each face in `split` by its children (same owner, neighbour
    /// and patch; the rings keep the parent's winding).
    pub fn apply_splits(&mut self, split: &BTreeMap<usize, Vec<Vec<usize>>>) {
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

    /// `decomposeFaces`: the fan of face `f` about a new point at its centre
    /// (OpenFOAM `face::centre`), children `[f[i], f[i+1], centre]`.
    pub fn fan(&mut self, f: usize) -> Vec<Vec<usize>> {
        let ring = self.faces[f].clone();
        let c = self.points.len();
        self.points.push(foam_face_centre(&self.points, &ring));
        (0..ring.len()).map(|i| vec![ring[i], ring[(i + 1) % ring.len()], c]).collect()
    }

    /// The surface of the mesh and its partition by the face patches
    /// (`None` is one more patch), from the shared engine (gh:#1257).
    pub fn surface(&self) -> (SurfaceEngine, SurfacePartitioner) {
        let unassigned = self.region.iter().flatten().max().map_or(0, |m| m + 1);
        let patch_of: Vec<usize> = self.region.iter().map(|r| r.unwrap_or(unassigned)).collect();
        let engine = SurfaceEngine::from_parts(&self.points, &self.faces, &self.owner, &self.neighbour, &patch_of);
        let part = SurfacePartitioner::new(&engine);
        (engine, part)
    }

    /// `decomposeFaces::decomposeMeshFaces`: fan every marked face about a
    /// new point at its centre, the new points numbered in face order.
    /// Returns the number of faces fanned.
    pub fn decompose_mesh_faces(&mut self, decompose_face: &[bool]) -> usize {
        let mut split = BTreeMap::new();
        for f in 0..self.faces.len() {
            if decompose_face[f] {
                let fan = self.fan(f);
                split.insert(f, fan);
            }
        }
        let n = split.len();
        self.apply_splits(&split);
        n
    }

    /// `decomposeFaces::decomposeConcaveInternalFaces`: an internal face
    /// with exactly one concave vertex is fanned from that vertex (no new
    /// point); one with more is fanned about a new centre point, children
    /// `[centre, f[i], f[i+1]]`. Boundary faces are kept. Unused points are
    /// removed afterwards, as upstream.
    pub fn decompose_concave_internal_faces(&mut self, concave_vertex: &[bool]) -> usize {
        let mut split = BTreeMap::new();
        for f in 0..self.faces.len() {
            if self.neighbour[f].is_none() {
                continue;
            }
            let ring = self.faces[f].clone();
            let k = ring.len();
            let pos: Vec<usize> = (0..k).filter(|&i| concave_vertex[ring[i]]).collect();
            if pos.len() == 1 {
                let c = pos[0];
                let children = (1..k - 1).map(|i| {
                    let j = (c + i) % k;
                    vec![ring[c], ring[j], ring[(j + 1) % k]]
                });
                split.insert(f, children.collect());
            } else if pos.len() > 1 {
                let c = self.points.len();
                self.points.push(foam_face_centre(&self.points, &ring));
                split.insert(f, (0..k).map(|i| vec![c, ring[i], ring[(i + 1) % k]]).collect());
            }
        }
        let n = split.len();
        self.apply_splits(&split);
        self.remove_unused_vertices();
        n
    }

    /// `polyMeshGenModifier::removeUnusedVertices`: drop points no face
    /// uses, renumbering the rest in order. Returns how many were removed.
    pub fn remove_unused_vertices(&mut self) -> usize {
        let mut used = vec![false; self.points.len()];
        for ring in &self.faces {
            for &v in ring {
                used[v] = true;
            }
        }
        let mut new_id = vec![usize::MAX; self.points.len()];
        let mut pts = Vec::new();
        for (v, &u) in used.iter().enumerate() {
            if u {
                new_id[v] = pts.len();
                pts.push(self.points[v]);
            }
        }
        let removed = self.points.len() - pts.len();
        if removed > 0 {
            for ring in &mut self.faces {
                for v in ring.iter_mut() {
                    *v = new_id[*v];
                }
            }
            self.points = pts;
        }
        removed
    }

    /// `polyMeshGenModifier::removeFaces`: drop the marked faces.
    pub fn remove_faces(&mut self, remove: &[bool]) {
        let keep: Vec<usize> = (0..self.faces.len()).filter(|&f| !remove[f]).collect();
        self.faces = keep.iter().map(|&f| self.faces[f].clone()).collect();
        self.owner = keep.iter().map(|&f| self.owner[f]).collect();
        self.neighbour = keep.iter().map(|&f| self.neighbour[f]).collect();
        self.region = keep.iter().map(|&f| self.region[f]).collect();
    }

    /// `polyMeshGenModifier::removeCells`: drop the marked cells. A face
    /// between a removed and a kept cell becomes a boundary face of the kept
    /// cell (reversed when the kept cell was its neighbour); faces of removed
    /// cells only are dropped; kept cells are renumbered in order. As
    /// upstream's `reorderBoundaryFaces`, every boundary face then lies in
    /// one patch: the existing one when the mesh had exactly one patch,
    /// else a new `defaultFaces` (returned `true` when that happened).
    pub fn remove_cells(&mut self, remove: &[bool]) -> bool {
        let mut new_id = vec![usize::MAX; self.n_cells];
        let mut n = 0usize;
        for c in 0..self.n_cells {
            if !remove[c] {
                new_id[c] = n;
                n += 1;
            }
        }
        // upstream: reverse, renumber the cells, `reorderBoundaryFaces` over
        // ALL faces (a face of removed cells only has no neighbour, so it is
        // swapped like a boundary face), then `removeFaces` (gh:#1385)
        let (mut faces, mut owner, mut neighbour, mut dead) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for f in 0..self.faces.len() {
            let o = self.owner[f];
            match self.neighbour[f] {
                Some(nb) => match (remove[o], remove[nb]) {
                    (false, false) => {
                        faces.push(self.faces[f].clone());
                        owner.push(new_id[o]);
                        neighbour.push(Some(new_id[nb]));
                        dead.push(false);
                    }
                    (true, false) => {
                        faces.push(reverse_face(&self.faces[f]));
                        owner.push(new_id[nb]);
                        neighbour.push(None);
                        dead.push(false);
                    }
                    (false, true) => {
                        faces.push(self.faces[f].clone());
                        owner.push(new_id[o]);
                        neighbour.push(None);
                        dead.push(false);
                    }
                    (true, true) => {
                        faces.push(self.faces[f].clone());
                        owner.push(0);
                        neighbour.push(None);
                        dead.push(true);
                    }
                },
                None => {
                    faces.push(self.faces[f].clone());
                    owner.push(if remove[o] { 0 } else { new_id[o] });
                    neighbour.push(None);
                    dead.push(remove[o]);
                }
            }
        }
        let carried = match &self.cells {
            Some(c) if crate::volume_mesh::cell_lists_are_consistent(c, self.n_cells, &self.owner, &self.neighbour) => Some(c.clone()),
            _ => None,
        };
        // `at[p]`: the original face now at position `p`
        let mut at: Vec<usize> = (0..faces.len()).collect();
        for (a, b) in crate::cartesian_mesh::boundary_swap_pairs(&neighbour) {
            faces.swap(a, b);
            owner.swap(a, b);
            neighbour.swap(a, b);
            dead.swap(a, b);
            at.swap(a, b);
        }
        let live: Vec<usize> = (0..faces.len()).filter(|&f| !dead[f]).collect();
        // the kept cells' lists keep their order; labels follow the swaps and
        // the removal of the dead faces (`reorderBoundaryFaces`, `removeFaces`)
        let new_cells: Option<Vec<Vec<usize>>> = carried.map(|c| {
            let mut final_label = vec![usize::MAX; faces.len()];
            for (n, &p) in live.iter().enumerate() {
                final_label[at[p]] = n;
            }
            (0..self.n_cells).filter(|&k| !remove[k]).map(|k| c[k].iter().map(|&f| final_label[f]).collect()).collect()
        });
        let faces: Vec<Vec<usize>> = live.iter().map(|&f| faces[f].clone()).collect();
        let owner: Vec<usize> = live.iter().map(|&f| owner[f]).collect();
        let neighbour: Vec<Option<usize>> = live.iter().map(|&f| neighbour[f]).collect();
        let mut region: Vec<Option<usize>> = vec![None; faces.len()];
        let collapsed = self.patch_names.len() != 1;
        if collapsed {
            self.patch_names = vec!["defaultFaces".to_string()];
        }
        for (r, nb) in region.iter_mut().zip(&neighbour) {
            if nb.is_none() {
                *r = Some(0);
            }
        }
        self.faces = faces;
        self.owner = owner;
        self.neighbour = neighbour;
        self.region = region;
        self.n_cells = n;
        self.cells = new_cells;
        collapsed
    }

    /// `polyMeshGenModifier::replaceBoundary`: keep the internal faces, in
    /// order, and replace every boundary face by the given faces, owners
    /// and patches.
    pub fn replace_boundary(&mut self, names: Vec<String>, faces: Vec<Vec<usize>>, owners: Vec<usize>, patches: Vec<usize>) {
        // upstream assumes the internal faces come first, and puts the new
        // boundary faces patch by patch; every cell list keeps its internal
        // faces and gets the new faces it owns appended in the order given
        // (gh:#1385)
        let n_internal = self.neighbour.iter().filter(|n| n.is_some()).count();
        let prefix = (0..n_internal).all(|f| self.neighbour[f].is_some());
        let carried = match &self.cells {
            Some(c) if prefix && crate::volume_mesh::cell_lists_are_consistent(c, self.n_cells, &self.owner, &self.neighbour) => Some(c.clone()),
            _ => None,
        };
        self.remove_faces(&self.neighbour.iter().map(|n| n.is_none()).collect::<Vec<bool>>());
        let mut start = vec![0usize; names.len() + 1];
        for &p in &patches {
            if p < names.len() {
                start[p + 1] += 1;
            }
        }
        for i in 0..names.len() {
            start[i + 1] += start[i];
        }
        let mut label_of = Vec::with_capacity(faces.len());
        let mut placed = vec![0usize; names.len()];
        let n_new = faces.len();
        let in_range = patches.iter().all(|&p| p < names.len());
        let mut slots: Vec<Option<(Vec<usize>, usize, usize)>> = vec![None; n_new];
        for (i, ((ring, o), p)) in faces.into_iter().zip(owners.iter().copied()).zip(patches.iter().copied()).enumerate() {
            let slot = if in_range {
                let s = start[p] + placed[p];
                placed[p] += 1;
                s
            } else {
                i
            };
            label_of.push(n_internal + slot);
            slots[slot] = Some((ring, o, p));
        }
        for (ring, o, p) in slots.into_iter().flatten() {
            self.faces.push(ring);
            self.owner.push(o);
            self.neighbour.push(None);
            self.region.push(Some(p));
        }
        self.cells = carried.map(|mut c| {
            for l in c.iter_mut() {
                l.retain(|&f| f < n_internal);
            }
            for (i, &o) in owners.iter().enumerate() {
                c[o].push(label_of[i]);
            }
            c
        });
        self.patch_names = names;
    }

    /// Mesh points on the boundary.
    fn boundary_vertex(&self) -> Vec<bool> {
        let mut bv = vec![false; self.points.len()];
        for f in 0..self.faces.len() {
            if self.neighbour[f].is_none() {
                for &v in &self.faces[f] {
                    bv[v] = true;
                }
            }
        }
        bv
    }

    /// Is every edge of cell `c` (faces `fs`) in exactly two of its faces?
    fn cell_is_closed(&self, fs: &[usize]) -> bool {
        let mut n: BTreeMap<(usize, usize), usize> = BTreeMap::new();
        for &f in fs {
            for (a, b) in ring_edges(&self.faces[f]) {
                *n.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        n.values().all(|&k| k == 2)
    }
}

// ---------------------------------------------------------------------------
// decomposeCells
// ---------------------------------------------------------------------------

/// cfMesh `decomposeCells::decomposeMesh` (gh:#1267): split every marked
/// cell into pyramids, one per face, apex at the average of the cell's
/// vertices. First `checkFaceConnections`: a face of a marked cell meeting
/// another face of that cell on more than one edge is fanned about its
/// centre (`decomposeMeshFaces`), so no two pyramids share more than one
/// face. Returns the number of faces fanned. The one shared copy: written
/// in `correct_edges` step 3, split out as `correct_edges::decompose_cells`
/// for `cartesian_mesh`'s `decomposeSplitHexes` (gh:#1266) and moved here
/// unchanged (gh:#1267; bit for bit,
/// `correct_edges::tests::correct_edges_is_pinned_bit_for_bit`). The
/// `topologicalCleaner` and `checkBoundaryFacesSharingTwoEdges` use it too.
/// Fan points are numbered as the faces are found (cell by cell), not in
/// face order as upstream; the geometry is the same.
pub fn decompose_cells(w: &mut TopoMesh, decompose_cell: &[bool]) -> usize {
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
    let n = split.len();
    w.apply_splits(&split);
    decompose_cells_into_pyramids(w, decompose_cell);
    n
}

/// [`decompose_cells`] and `decomposeFaces` on a bare mesh (gh:#1266,
/// `cartesianMeshExtractor::decomposeSplitHexesIntoTetsAndPyramids`): fan
/// the faces in `fan_faces` about their OpenFOAM centre, then decompose the
/// marked cells. Returns the new points, faces, owners, neighbours and cell
/// count; boundary faces keep their order after the internal faces. Moved
/// here from `correct_edges` unchanged (gh:#1267).
#[allow(clippy::type_complexity)]
pub(crate) fn decompose_faces_and_cells(
    points: Vec<Vec3>,
    faces: Vec<Vec<usize>>,
    owner: Vec<usize>,
    neighbour: Vec<Option<usize>>,
    n_cells: usize,
    fan_faces: &[bool],
    decompose_cell: &[bool],
) -> (Vec<Vec3>, Vec<Vec<usize>>, Vec<usize>, Vec<Option<usize>>, usize) {
    let region = vec![None; faces.len()];
    let mut w = TopoMesh { points, faces, owner, neighbour, region, n_cells, patch_names: Vec::new(), cells: None };
    let mut split = BTreeMap::new();
    for (f, &d) in fan_faces.iter().enumerate() {
        if d {
            let fan = w.fan(f);
            split.insert(f, fan);
        }
    }
    w.apply_splits(&split);
    if decompose_cell.iter().any(|&d| d) {
        decompose_cells(&mut w, decompose_cell);
    }
    (w.points, w.faces, w.owner, w.neighbour, w.n_cells)
}

/// `decomposeCellIntoPyramids` for every marked cell: one pyramid per face,
/// apex at the average of the cell's vertices (`findTopVertex`). Unmarked
/// cells keep their order (renumbered past the removed ones); the pyramids
/// are appended cell by cell, face by face (`removeDecomposedCells`,
/// `addNewCells`). Internal faces keep owner < neighbour. Moved here
/// unchanged from `correct_edges` (gh:#1267).
pub fn decompose_cells_into_pyramids(w: &mut TopoMesh, mark: &[bool]) {
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

// ---------------------------------------------------------------------------
// faceDecomposition and the face helpers
// ---------------------------------------------------------------------------

/// `help::sortEdgeChain`: order directed edges head to tail from the first
/// and return each one's start point.
pub fn sort_edge_chain(edges: &[(usize, usize)]) -> Result<Vec<usize>, TopologyError> {
    let mut sorted = vec![false; edges.len()];
    let mut chain = vec![edges[0]];
    sorted[0] = true;
    let mut i = 0usize;
    loop {
        let mut finished = true;
        for e in 0..edges.len() {
            if sorted[e] {
                continue;
            }
            if chain[i].1 == edges[e].0 {
                sorted[e] = true;
                finished = false;
                chain.push(edges[e]);
                i += 1;
            } else if chain[i].1 == edges[e].1 {
                return Err(TopologyError::ChainNotOriented);
            }
        }
        if finished {
            break;
        }
    }
    if chain.len() != edges.len() {
        return Err(TopologyError::OpenChain);
    }
    Ok(chain.iter().map(|e| e.0).collect())
}

/// `help::mergeTwoFaces`: the face bounded by the edges of `f1` and `f2`
/// that they do not share.
pub fn merge_two_faces(f1: &[usize], f2: &[usize]) -> Result<Vec<usize>, TopologyError> {
    let (e1, e2) = (ring_edges(f1), ring_edges(f2));
    let (mut ce1, mut ce2) = (vec![false; e1.len()], vec![false; e2.len()]);
    for (i, &a) in e1.iter().enumerate() {
        if let Some(j) = e2.iter().position(|&b| same_edge(b, a.0, a.1)) {
            ce1[i] = true;
            ce2[j] = true;
        }
    }
    let mut kept: Vec<(usize, usize)> = (0..e1.len()).filter(|&i| !ce1[i]).map(|i| e1[i]).collect();
    kept.extend((0..e2.len()).filter(|&i| !ce2[i]).map(|i| e2[i]));
    sort_edge_chain(&kept)
}

/// `help::removeEdgesFromFace`: drop the points of `f` between two of the
/// given edges.
pub(crate) fn remove_edges_from_face(f: &[usize], remove: &[(usize, usize)]) -> Vec<usize> {
    let k = f.len();
    let mut found = vec![false; k];
    for &e in remove {
        if let Some(i) = (0..k).find(|&i| same_edge(e, f[i], f[(i + 1) % k])) {
            found[i] = true;
        }
    }
    (0..k).filter(|&p| !(found[p] && found[(p + k - 1) % k])).map(|p| f[p]).collect()
}

/// `help::createFaceFromRemovedPart`: the part of `orig` that `cut` lost.
pub(crate) fn create_face_from_removed_part(orig: &[usize], cut: &[usize]) -> Vec<usize> {
    if cut.is_empty() {
        return orig.to_vec();
    }
    let k = orig.len();
    let ce = ring_edges(cut);
    let used: Vec<bool> = (0..k).map(|i| ce.iter().any(|&e| same_edge(e, orig[i], orig[(i + 1) % k]))).collect();
    (0..k).filter(|&p| !(used[p] && used[(p + k - 1) % k])).map(|p| orig[p]).collect()
}

/// `help::shareAnEdge`.
fn share_an_edge(f1: &[usize], f2: &[usize]) -> bool {
    let e2 = ring_edges(f2);
    ring_edges(f1).iter().any(|&a| e2.iter().any(|&b| same_edge(b, a.0, a.1)))
}

/// cfMesh `faceDecomposition::concaveVertex`: the vertex (mesh label) where
/// the turn from one edge to the next is against the face normal by more
/// than `SMALL`; `None` for a convex face.
pub fn concave_vertex(ring: &[usize], points: &[Vec3]) -> Result<Option<usize>, TopologyError> {
    let n = ring_area_vector(points, ring).normalize();
    let edges = ring_edges(ring);
    let k = ring.len();
    let mut cv = None;
    for e in 0..k {
        let next = (e + 1) % k;
        let ev = points[edges[e].1].sub(points[edges[e].0]).normalize();
        let evn = points[edges[next].1].sub(points[edges[next].0]).normalize();
        if ev.cross(evn).dot(n) < -SMALL {
            if cv.is_some() {
                return Err(TopologyError::MoreThanOneConcaveVertex);
            }
            cv = Some(edges[e].1);
        }
    }
    Ok(cv)
}

/// `faceDecomposition::isFaceConvex`.
pub fn is_face_convex(ring: &[usize], points: &[Vec3]) -> Result<bool, TopologyError> {
    Ok(concave_vertex(ring, points)?.is_none())
}

/// `faceDecomposition::isFacePlanar(tol)`: every point within `tol` of the
/// plane through the first point, normal to the face.
pub fn is_face_planar_tol(ring: &[usize], points: &[Vec3], tol: f64) -> bool {
    let n = ring_area_vector(points, ring).normalize();
    ring.iter().all(|&v| points[v].sub(points[ring[0]]).dot(n).abs() <= tol)
}

/// `faceDecomposition::isFacePlanar()`: tolerance 5 % of the largest
/// centre-to-point distance.
pub fn is_face_planar(ring: &[usize], points: &[Vec3]) -> bool {
    let c = foam_face_centre(points, ring);
    let tol = ring.iter().fold(0.0f64, |t, &v| t.max(c.sub(points[v]).length())) * 0.05;
    is_face_planar_tol(ring, points, tol)
}

/// `faceDecomposition::decomposeFaceIntoTriangles`: a fan from the concave
/// vertex, or the face itself when it is convex.
pub fn decompose_face_into_triangles(ring: &[usize], points: &[Vec3]) -> Result<Vec<Vec<usize>>, TopologyError> {
    let Some(cv) = concave_vertex(ring, points)? else { return Ok(vec![ring.to_vec()]) };
    let k = ring.len();
    let start = ring.iter().position(|&v| v == cv).unwrap_or(0);
    Ok((1..k - 1)
        .map(|e| {
            let i = (e + start) % k;
            vec![ring[start], ring[i], ring[(i + 1) % k]]
        })
        .collect())
}

/// `faceDecomposition::decomposeFace`: the triangles of
/// [`decompose_face_into_triangles`] merged from both ends while the merge
/// stays convex, into the fewest convex faces upstream's sweep finds.
pub fn decompose_face(ring: &[usize], points: &[Vec3]) -> Result<Vec<Vec<usize>>, TopologyError> {
    let ff = decompose_face_into_triangles(ring, points)?;
    if ff.len() <= 3 {
        return Ok(ff);
    }
    let convex = |f: &[usize]| is_face_convex(f, points);
    let mut lf = ff[0].clone();
    let mut rf = ff[ff.len() - 1].clone();
    let (mut il, mut ir) = (1usize, ff.len() - 2);
    let mut storage: Vec<Vec<usize>> = Vec::new();
    while il < ir {
        let fl = merge_two_faces(&ff[il], &lf)?;
        if convex(&fl)? {
            lf = fl;
            if il == ir - 1 {
                storage.push(lf.clone());
            }
        } else {
            storage.push(lf.clone());
            lf = ff[il].clone();
            if il == ir - 1 {
                storage.push(lf.clone());
            }
        }
        let fr = merge_two_faces(&rf, &ff[ir])?;
        if convex(&fr)? {
            rf = fr;
            if il == ir - 1 {
                storage.push(rf.clone());
            }
        } else {
            storage.push(rf.clone());
            rf = ff[ir].clone();
            if il == ir - 1 {
                storage.push(rf.clone());
            }
        }
        il += 1;
        ir -= 1;
        if il == ir {
            let fl = merge_two_faces(&ff[il], &lf)?;
            let fr = merge_two_faces(&rf, &ff[ir])?;
            if convex(&fl)? {
                storage.push(fl);
                storage.push(rf.clone());
            } else if convex(&fr)? {
                storage.push(fr);
                storage.push(lf.clone());
            } else {
                storage.push(lf.clone());
                storage.push(ff[il].clone());
                storage.push(rf.clone());
            }
        }
    }
    Ok(storage)
}

// ---------------------------------------------------------------------------
// surfaceMorpherCells
// ---------------------------------------------------------------------------

/// What [`surface_morpher_cells`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SurfaceMorpherReport {
    /// Passes of `morphMesh`'s loop.
    pub iterations: usize,
    /// Cells removed (all vertices on the boundary, or not closed).
    pub n_cells_removed: usize,
    /// Internal faces shrunk by `morphInternalFaces`.
    pub n_internal_faces_morphed: usize,
    /// Internal faces removed (fewer than three points left).
    pub n_internal_faces_removed: usize,
    /// Boundary faces merged into a neighbour by `morphBoundaryFaces`.
    pub n_boundary_faces_merged: usize,
}

/// cfMesh `surfaceMorpherCells::morphMesh` (gh:#1267), the first step of
/// `cartesianMesh`'s surface preparation: repeat, until nothing changes,
/// (1) remove boundary cells with every vertex on the boundary or not
/// closed, (2) shrink each internal face by every vertex whose two face
/// neighbours and itself are all on the boundary, the cut-off part becoming
/// a boundary face of both cells, (3) merge the boundary faces of a cell
/// that share a chain of edges. Then remove unused points. The boundary is
/// one patch, `defaultFaces`, after any of (2) or (3) (upstream's
/// `replaceMeshBoundary`). Upstream has no iteration limit; this port stops
/// with [`TopologyError::NoConvergence`] after 10 000 passes.
pub fn surface_morpher_cells(w: &mut TopoMesh) -> Result<SurfaceMorpherReport, TopologyError> {
    let mut rep = SurfaceMorpherReport::default();
    loop {
        rep.iterations += 1;
        if rep.iterations > 10_000 {
            return Err(TopologyError::NoConvergence);
        }
        let bv = w.boundary_vertex();
        let mut boundary_cell = vec![false; w.n_cells];
        for f in 0..w.faces.len() {
            if w.neighbour[f].is_none() {
                boundary_cell[w.owner[f]] = true;
            }
        }
        if remove_cells_with_all_vertices_at_the_boundary(w, &bv, &boundary_cell, &mut rep) {
            continue;
        }
        if morph_internal_faces(w, &bv, &mut rep) {
            continue;
        }
        if morph_boundary_faces(w, &boundary_cell, &mut rep)? {
            continue;
        }
        break;
    }
    w.remove_unused_vertices();
    Ok(rep)
}

/// `removeCellsWithAllVerticesAtTheBoundary`.
fn remove_cells_with_all_vertices_at_the_boundary(w: &mut TopoMesh, bv: &[bool], boundary_cell: &[bool], rep: &mut SurfaceMorpherReport) -> bool {
    let cf = w.cell_faces();
    let mut remove = vec![false; w.n_cells];
    for c in 0..w.n_cells {
        if !boundary_cell[c] {
            continue;
        }
        if cf[c].iter().all(|&f| w.faces[f].iter().all(|&v| bv[v])) || !w.cell_is_closed(&cf[c]) {
            remove[c] = true;
        }
    }
    let n = remove.iter().filter(|&&r| r).count();
    if n > 0 {
        rep.n_cells_removed += n;
        w.remove_cells(&remove);
    }
    n > 0
}

/// `morphInternalFaces`.
fn morph_internal_faces(w: &mut TopoMesh, bv: &[bool], rep: &mut SurfaceMorpherReport) -> bool {
    let mut nb_faces: Vec<Vec<usize>> = Vec::new();
    let mut nb_owner: Vec<usize> = Vec::new();
    for f in 0..w.faces.len() {
        if w.neighbour[f].is_none() {
            nb_faces.push(w.faces[f].clone());
            nb_owner.push(w.owner[f]);
        }
    }
    let mut changed = false;
    for f in 0..w.faces.len() {
        let Some(nb) = w.neighbour[f] else { continue };
        let ring = w.faces[f].clone();
        let k = ring.len();
        let remove_v: Vec<bool> = (0..k).map(|p| bv[ring[(p + k - 1) % k]] && bv[ring[p]] && bv[ring[(p + 1) % k]]).collect();
        let new_f: Vec<usize> = (0..k).filter(|&p| !remove_v[p]).map(|p| ring[p]).collect();
        if new_f.len() == k {
            continue;
        }
        changed = true;
        rep.n_internal_faces_morphed += 1;
        // Group the removed points into runs (materials), each with its edges.
        let mut node_mat = vec![0usize; k];
        let mut edge_mats: Vec<Vec<(usize, usize)>> = Vec::new();
        let mut mat = 1usize;
        for n0 in 0..k {
            if node_mat[n0] != 0 || !remove_v[n0] {
                continue;
            }
            edge_mats.push(Vec::new());
            let mut front = vec![n0];
            while !front.is_empty() {
                let mut new_front = Vec::new();
                for &p in &front {
                    if node_mat[p] != 0 {
                        continue;
                    }
                    node_mat[p] = mat;
                    let em = &mut edge_mats[mat - 1];
                    for e in [(ring[p], ring[(p + 1) % k]), (ring[(p + k - 1) % k], ring[p])] {
                        if !em.iter().any(|&x| same_edge(x, e.0, e.1)) {
                            em.push(e);
                        }
                    }
                    if remove_v[(p + k - 1) % k] {
                        new_front.push((p + k - 1) % k);
                    }
                    if remove_v[(p + 1) % k] {
                        new_front.push((p + 1) % k);
                    }
                }
                front = new_front;
            }
            mat += 1;
        }
        for em in &edge_mats {
            let rf = remove_edges_from_face(&ring, em);
            let new_bf = create_face_from_removed_part(&ring, &rf);
            nb_faces.push(new_bf.iter().rev().copied().collect());
            nb_owner.push(nb);
            nb_faces.push(new_bf.clone());
            nb_owner.push(w.owner[f]);
            // Upstream appends the owner's face first, then the neighbour's.
            let n = nb_faces.len();
            nb_faces.swap(n - 1, n - 2);
            nb_owner.swap(n - 1, n - 2);
        }
        w.faces[f] = new_f;
    }
    if changed {
        let n = nb_faces.len();
        w.replace_boundary(vec!["defaultFaces".to_string()], nb_faces, nb_owner, vec![0; n]);
        let remove: Vec<bool> = (0..w.faces.len()).map(|f| w.neighbour[f].is_some() && w.faces[f].len() < 3).collect();
        let nr = remove.iter().filter(|&&r| r).count();
        if nr > 0 {
            rep.n_internal_faces_removed += nr;
            w.remove_faces(&remove);
        }
    }
    changed
}

/// `morphBoundaryFaces`.
fn morph_boundary_faces(w: &mut TopoMesh, boundary_cell: &[bool], rep: &mut SurfaceMorpherReport) -> Result<bool, TopologyError> {
    let cf = w.cell_faces();
    let (mut nb_faces, mut nb_owner) = (Vec::new(), Vec::new());
    let mut changed = false;
    for c in 0..w.n_cells {
        if !boundary_cell[c] {
            continue;
        }
        let bfaces: Vec<usize> = cf[c].iter().copied().filter(|&f| w.neighbour[f].is_none()).collect();
        let mut merged = vec![false; bfaces.len()];
        let mut mf = w.faces[bfaces[0]].clone();
        merged[0] = true;
        loop {
            let mut finished = true;
            for i in 1..bfaces.len() {
                if merged[i] {
                    continue;
                }
                let bf = &w.faces[bfaces[i]];
                let (be, me) = (ring_edges(bf), ring_edges(&mf));
                let n_shared_edges = be.iter().map(|&a| me.iter().filter(|&&b| same_edge(b, a.0, a.1)).count()).sum::<usize>();
                let n_shared_points = bf.iter().map(|&a| mf.iter().filter(|&&b| b == a).count()).sum::<usize>();
                if n_shared_edges > 0 && n_shared_edges + 1 == n_shared_points {
                    mf = merge_two_faces(&mf, bf)?;
                    merged[i] = true;
                    changed = true;
                    finished = false;
                    rep.n_boundary_faces_merged += 1;
                }
            }
            if finished {
                break;
            }
        }
        nb_faces.push(mf);
        nb_owner.push(c);
        for i in 1..bfaces.len() {
            if !merged[i] {
                nb_faces.push(w.faces[bfaces[i]].clone());
                nb_owner.push(c);
            }
        }
    }
    if changed {
        let n = nb_faces.len();
        w.replace_boundary(vec!["defaultFaces".to_string()], nb_faces, nb_owner, vec![0; n]);
    }
    Ok(changed)
}

// ---------------------------------------------------------------------------
// topologicalCleaner
// ---------------------------------------------------------------------------

/// What [`topological_cleaner`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TopologicalCleanerReport {
    /// Boundary points split because their cells form more than one group
    /// (`checkInvalidConnectionsForVerticesCells`).
    pub n_invalid_vertex_cell_connections: usize,
    /// Cells removed at boundary edges with more than two boundary faces
    /// (`checkInvalidConnectionsForVerticesFaces`).
    pub n_cells_removed_at_edges: usize,
    /// Boundary faces fanned by `checkNonConsecutiveBoundaryVertices`.
    pub n_non_consecutive_faces: usize,
    /// Cells with more than one boundary face (`checkNonMappableCells`).
    pub n_non_mappable_cells: usize,
    /// Internal faces fanned by `checkNonMappableFaces`.
    pub n_non_mappable_faces: usize,
    /// Cells split into pyramids.
    pub n_decomposed_cells: usize,
    /// Cells removed by `checkCellConnectionsOverFaces`.
    pub n_cells_removed_unconnected: usize,
    /// Upstream's return value.
    pub changed: bool,
}

/// cfMesh `topologicalCleaner::cleanTopology` (gh:#1267), in upstream's
/// order: split boundary points shared by unconnected cell groups; remove
/// the cells at boundary edges with more than two boundary faces; fan the
/// boundary faces sharing non-consecutive points with an internal face of
/// their cell, and mark that cell; mark cells with more than one boundary
/// face; fan internal faces with more than two boundary points or two that
/// are not consecutive, and mark both cells; decompose the marked cells
/// ([`decompose_cells`]); keep the largest face-connected cell group. As
/// upstream, `changed` is set by the fanning and marking steps and by the
/// last one only.
pub fn topological_cleaner(w: &mut TopoMesh) -> TopologicalCleanerReport {
    let mut rep = TopologicalCleanerReport::default();
    let mut changed = false;
    rep.n_invalid_vertex_cell_connections = check_invalid_connections_for_vertices_cells(w);
    let mut decompose_cell = vec![false; w.n_cells];

    // checkInvalidConnectionsForVerticesFaces.
    {
        let (engine, _) = w.surface();
        let mut remove = vec![false; w.n_cells];
        let mut any = false;
        for ef in &engine.edge_faces {
            if ef.len() > 2 {
                for &bf in ef {
                    remove[engine.face_owners[bf]] = true;
                }
                any = true;
            }
        }
        if any {
            rep.n_cells_removed_at_edges = remove.iter().filter(|&&r| r).count();
            w.remove_cells(&remove);
            decompose_cell = vec![false; w.n_cells];
        }
    }

    // checkNonConsecutiveBoundaryVertices.
    {
        let cf = w.cell_faces();
        let mut decompose_face = vec![false; w.faces.len()];
        for f in 0..w.faces.len() {
            if w.neighbour[f].is_some() {
                continue;
            }
            let bf = &w.faces[f];
            let k = bf.len();
            for &g in &cf[w.owner[f]] {
                if w.neighbour[g].is_none() {
                    continue;
                }
                let sh: Vec<usize> = (0..k).filter(|&p| w.faces[g].contains(&bf[p])).collect();
                let bad = sh.len() > 2 || (sh.len() == 2 && !(sh[0] == (sh[1] + 1) % k || sh[0] == (sh[1] + k - 1) % k));
                if bad {
                    decompose_face[f] = true;
                    decompose_cell[w.owner[f]] = true;
                    changed = true;
                }
            }
        }
        if decompose_face.iter().any(|&d| d) {
            rep.n_non_consecutive_faces = w.decompose_mesh_faces(&decompose_face);
        }
    }

    // checkNonMappableCells.
    {
        let mut nb = vec![0usize; w.n_cells];
        for f in 0..w.faces.len() {
            if w.neighbour[f].is_none() {
                nb[w.owner[f]] += 1;
            }
        }
        for c in 0..w.n_cells {
            if nb[c] > 1 {
                rep.n_non_mappable_cells += 1;
                decompose_cell[c] = true;
            }
        }
        if rep.n_non_mappable_cells != 0 {
            changed = true;
        }
    }

    // checkNonMappableFaces.
    {
        let bv = w.boundary_vertex();
        let mut decompose_face = vec![false; w.faces.len()];
        let mut n_bad = 0usize;
        for f in 0..w.faces.len() {
            let Some(nb) = w.neighbour[f] else { continue };
            let ring = &w.faces[f];
            let bpos: Vec<usize> = (0..ring.len()).filter(|&p| bv[ring[p]]).collect();
            if bpos.len() > 2 || (bpos.len() == 2 && bpos[1] != bpos[0] + 1 && !(bpos[0] == 0 && bpos[1] == ring.len() - 1)) {
                n_bad += 1;
                decompose_face[f] = true;
                decompose_cell[w.owner[f]] = true;
                decompose_cell[nb] = true;
            }
        }
        if n_bad != 0 {
            changed = true;
            rep.n_non_mappable_faces = w.decompose_mesh_faces(&decompose_face);
        }
    }

    // decomposeCells.
    if changed {
        rep.n_decomposed_cells = decompose_cell.iter().filter(|&&d| d).count();
        decompose_cells(w, &decompose_cell);
    }

    let before = w.n_cells;
    if check_cell_connections_over_faces(w) {
        changed = true;
        rep.n_cells_removed_unconnected = before - w.n_cells;
    }
    rep.changed = changed;
    rep
}

/// `topologicalCleaner::checkInvalidConnectionsForVerticesCells`: a
/// boundary point whose cells form more than one group (connected through
/// their faces) gets one copy per extra group. Returns the points split.
fn check_invalid_connections_for_vertices_cells(w: &mut TopoMesh) -> usize {
    let point_cells = w.point_cells();
    let cell_cells = w.cell_cells();
    let cf = w.cell_faces();
    let bv = w.boundary_vertex();
    let mut n_invalid = 0usize;
    for p in 0..bv.len() {
        if !bv[p] {
            continue;
        }
        let pc = &point_cells[p];
        let mut material_of = vec![0usize; pc.len()];
        let mut material = 1usize;
        for c0 in 0..pc.len() {
            if material_of[c0] != 0 {
                continue;
            }
            material_of[c0] = material;
            let mut front = vec![c0];
            while !front.is_empty() {
                let mut new_front = Vec::new();
                for &fc in &front {
                    for &nc in &cell_cells[pc[fc]] {
                        if let Some(j) = (0..pc.len()).find(|&j| material_of[j] == 0 && pc[j] == nc) {
                            new_front.push(j);
                            material_of[j] = material;
                        }
                    }
                }
                front = new_front;
            }
            material += 1;
        }
        if material > 2 {
            n_invalid += 1;
            let n_points = w.points.len();
            for (j, &c) in pc.iter().enumerate() {
                if material_of[j] == 1 {
                    continue;
                }
                for &f in &cf[c] {
                    for v in w.faces[f].iter_mut() {
                        if *v == p {
                            *v = n_points + material_of[j] - 1;
                        }
                    }
                }
            }
            for _ in 1..material {
                let x = w.points[p];
                w.points.push(x);
            }
        }
    }
    if n_invalid != 0 {
        w.remove_unused_vertices();
    }
    n_invalid
}

// ---------------------------------------------------------------------------
// The cartesianMesh connectivity checks (gh:#1268)
// ---------------------------------------------------------------------------

/// cfMesh `checkCellConnectionsOverFaces::checkCellGroups`: group the cells
/// connected over faces (`help::groupMarking`, groups numbered from the
/// lowest cell); with more than one group keep the one with the most cells
/// (the first, on a tie) and remove the rest. Returns whether cells were
/// removed.
pub fn check_cell_connections_over_faces(w: &mut TopoMesh) -> bool {
    let (group, n_groups) = cell_groups(w);
    if n_groups == 1 {
        return false;
    }
    let mut n_in = vec![0usize; n_groups];
    for &g in &group {
        n_in[g] += 1;
    }
    let mut keep = 0usize;
    let mut best: i64 = -1;
    for (g, &n) in n_in.iter().enumerate() {
        if n as i64 > best {
            best = n as i64;
            keep = g;
        }
    }
    let remove: Vec<bool> = group.iter().map(|&g| g != keep).collect();
    w.remove_cells(&remove);
    true
}

/// Face-connected cell groups: `help::groupMarking` as ported in
/// [`crate::mesh_octree_creator::group_marking`] (gh:#1264), with
/// `meshConnectionsNeighbourOperator` (the cells across each face) and the
/// select-all `meshConnectionsSelectorOperator`.
fn cell_groups(w: &TopoMesh) -> (Vec<usize>, usize) {
    let cc = w.cell_cells();
    let (group, n) = crate::mesh_octree_creator::group_marking(w.n_cells, |c| cc[c].clone(), |_| true);
    (group.into_iter().map(|g| g.expect("every cell is selected")).collect(), n)
}

/// cfMesh `checkNonMappableCellConnections` cell types.
const INTERNALCELL: u8 = 1;
const BNDCELL: u8 = 2;
const ALLBNDVERTEXCELL: u8 = 4;
const INTERNALFACEGROUP: u8 = 8;

/// cfMesh `checkNonMappableCellConnections::findCells`: boundary cells with
/// every vertex on the boundary whose internal faces fall into more than one
/// edge-connected group, and those with all vertices on the boundary that
/// touch an internal cell or have a single neighbour.
pub fn non_mappable_cells(w: &TopoMesh) -> BTreeSet<usize> {
    let cf = w.cell_faces();
    let mut cell_type = vec![INTERNALCELL; w.n_cells];
    for f in 0..w.faces.len() {
        if w.neighbour[f].is_none() {
            cell_type[w.owner[f]] = BNDCELL;
        }
    }
    let bv = w.boundary_vertex();
    for c in (0..w.n_cells).rev() {
        if cell_type[c] & INTERNALCELL != 0 {
            continue;
        }
        if cf[c].iter().all(|&f| w.faces[f].iter().all(|&v| bv[v])) {
            cell_type[c] |= ALLBNDVERTEXCELL;
        } else {
            continue;
        }
        let internal: Vec<usize> = cf[c].iter().copied().filter(|&f| w.neighbour[f].is_some()).collect();
        let mut group: BTreeMap<usize, usize> = BTreeMap::new();
        let mut n_group = 0usize;
        for &f0 in &internal {
            if group.contains_key(&f0) {
                continue;
            }
            group.insert(f0, n_group);
            let mut front = vec![f0];
            while let Some(fl) = front.pop() {
                for &nf in &internal {
                    if !group.contains_key(&nf) && share_an_edge(&w.faces[fl], &w.faces[nf]) {
                        front.push(nf);
                        group.insert(nf, n_group);
                    }
                }
            }
            n_group += 1;
        }
        if n_group > 1 {
            cell_type[c] |= INTERNALFACEGROUP;
        }
    }
    let mut bad = BTreeSet::new();
    for c in (0..w.n_cells).rev() {
        if cell_type[c] & INTERNALFACEGROUP != 0 {
            bad.insert(c);
        } else if cell_type[c] & (ALLBNDVERTEXCELL + INTERNALFACEGROUP) != 0 {
            let mut has_internal_neighbour = false;
            let mut n_nei = 0usize;
            for &f in &cf[c] {
                let Some(nb) = w.neighbour[f] else { continue };
                n_nei += 1;
                let other = if nb == c { w.owner[f] } else { nb };
                if cell_type[other] & INTERNALCELL != 0 {
                    has_internal_neighbour = true;
                    break;
                }
            }
            if has_internal_neighbour || n_nei == 1 {
                bad.insert(c);
            }
        }
    }
    bad
}

/// cfMesh `checkNonMappableCellConnections::removeCells`: remove
/// [`non_mappable_cells`] until there are none. Returns the cells removed.
pub fn remove_non_mappable_cells(w: &mut TopoMesh) -> usize {
    let mut total = 0usize;
    loop {
        let bad = non_mappable_cells(w);
        if bad.is_empty() {
            return total;
        }
        total += bad.len();
        let remove: Vec<bool> = (0..w.n_cells).map(|c| bad.contains(&c)).collect();
        w.remove_cells(&remove);
    }
}

/// cfMesh `checkBoundaryFacesSharingTwoEdges`: boundary points in exactly
/// two boundary faces (the faces then share the two edges at the point).
#[derive(Debug, Clone, Copy, Default)]
pub struct BoundaryFacesSharingTwoEdges;

impl BoundaryFacesSharingTwoEdges {
    /// `findPoints`: the mesh points in exactly two boundary faces.
    pub fn find_points(w: &TopoMesh) -> BTreeSet<usize> {
        let (engine, _) = w.surface();
        (0..engine.boundary_points.len()).filter(|&b| engine.point_faces[b].len() == 2).map(|b| engine.boundary_points[b]).collect()
    }

    /// `improveTopology`: remove each such point that every internal face
    /// through it holds between two boundary edges (`findFacesAtBndEdge`,
    /// `removeExcessiveVertices`); fan every boundary face that still holds
    /// one (`findBndFacesForDecomposition`, its points and counts from
    /// before the removal, as upstream's cached surface). The cells are not
    /// decomposed: see the module documentation (upstream defect). Returns
    /// (points removed, boundary faces fanned).
    pub fn improve_topology(w: &mut TopoMesh) -> (usize, usize) {
        let (engine, _) = w.surface();
        let nbp = engine.boundary_points.len();
        let n_bnd: Vec<usize> = engine.point_faces.iter().map(|pf| pf.len()).collect();
        let edge_is = |b: usize, a: usize, c: usize| engine.point_edges[b].iter().any(|&e| same_edge((engine.edges[e][0], engine.edges[e][1]), a, c));
        // findFacesAtBndEdge.
        let mut remove_bp = vec![true; nbp];
        for f in 0..w.faces.len() {
            if w.neighbour[f].is_none() {
                continue;
            }
            let ring = &w.faces[f];
            let k = ring.len();
            for p in 0..k {
                let b = engine.bp[ring[p]];
                if b == NOT_BOUNDARY {
                    continue;
                }
                if n_bnd[b] == 2 {
                    let prev = edge_is(b, ring[(p + k - 1) % k], ring[p]);
                    let next = edge_is(b, ring[p], ring[(p + 1) % k]);
                    if !(prev && next) {
                        remove_bp[b] = false;
                    }
                } else {
                    remove_bp[b] = false;
                }
            }
        }
        // removeExcessiveVertices.
        let drop = |v: usize| engine.bp[v] != NOT_BOUNDARY && remove_bp[engine.bp[v]] && n_bnd[engine.bp[v]] == 2;
        let n_removed = (0..nbp).filter(|&b| remove_bp[b] && n_bnd[b] == 2).count();
        for ring in w.faces.iter_mut() {
            if ring.iter().any(|&v| drop(v)) {
                ring.retain(|&v| !drop(v));
            }
        }
        // findBndFacesForDecomposition.
        let mut decompose_face = vec![false; w.faces.len()];
        let mut n_decomposed = 0usize;
        for f in 0..w.faces.len() {
            if w.neighbour[f].is_some() {
                continue;
            }
            for &v in &w.faces[f] {
                let b = engine.bp[v];
                if b != NOT_BOUNDARY && n_bnd[b] == 2 {
                    n_decomposed += 1;
                    decompose_face[f] = true;
                }
            }
        }
        let mut n_fanned = 0;
        if n_decomposed != 0 {
            n_fanned = w.decompose_mesh_faces(&decompose_face);
            // decomposeCells::decomposeMesh with no cell marked (upstream).
            decompose_cells(w, &vec![false; w.n_cells]);
        }
        w.remove_unused_vertices();
        (n_removed, n_fanned)
    }
}

/// What [`check_and_fix_irregular_connections`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IrregularConnectionsReport {
    /// Boundary points found irregular (`badVertices`), mesh labels at the
    /// time each was found.
    pub bad_vertices: BTreeSet<usize>,
    /// Points split by `checkAndFixCellGroupsAtBndVertices`.
    pub n_points_split: usize,
    /// Cells removed by `checkEdgeFaceConnections`.
    pub n_cells_removed_at_edges: usize,
    /// Cells removed by `checkFaceGroupsAtBndVertices`.
    pub n_cells_removed_at_vertices: usize,
}

/// `checkIrregularSurfaceConnections::checkAndFixCellGroupsAtBndVertices`:
/// a boundary point whose cells form more than one group (connected over
/// faces, among the point's cells) is given one copy per extra group when
/// `fix`. Returns whether a point was bad.
fn cell_groups_at_bnd_vertices(w: &mut TopoMesh, bad: &mut BTreeSet<usize>, fix: bool, rep: &mut IrregularConnectionsReport) -> bool {
    let (engine, _) = w.surface();
    let point_cells = w.point_cells();
    let cell_cells = w.cell_cells();
    let cf = w.cell_faces();
    let mut n_bad = 0usize;
    for &p in &engine.boundary_points {
        let pc = &point_cells[p];
        let mut group: BTreeMap<usize, usize> = BTreeMap::new();
        let mut n_group = 0usize;
        for &c0 in pc {
            if group.contains_key(&c0) {
                continue;
            }
            group.insert(c0, n_group);
            let mut front = vec![c0];
            while let Some(c) = front.pop() {
                for &nc in &cell_cells[c] {
                    if !group.contains_key(&nc) && pc.binary_search(&nc).is_ok() {
                        group.insert(nc, n_group);
                        front.push(nc);
                    }
                }
            }
            n_group += 1;
        }
        if n_group != 1 {
            n_bad += 1;
            bad.insert(p);
            if fix {
                rep.n_points_split += 1;
                let n_points = w.points.len();
                for &c in pc {
                    if group[&c] != 0 {
                        for &f in &cf[c] {
                            if let Some(pos) = w.faces[f].iter().position(|&v| v == p) {
                                w.faces[f][pos] = n_points + group[&c] - 1;
                            }
                        }
                    }
                }
                for _ in 1..n_group {
                    let x = w.points[p];
                    w.points.push(x);
                }
            }
        }
    }
    n_bad != 0
}

/// `checkIrregularSurfaceConnections::checkEdgeFaceConnections`: boundary
/// edges in more than two boundary faces; with `fix`, remove every cell
/// with a face holding such an edge. Returns whether cells were removed.
fn edge_face_connections(w: &mut TopoMesh, bad: &mut BTreeSet<usize>, fix: bool, rep: &mut IrregularConnectionsReport) -> bool {
    let (engine, _) = w.surface();
    let bad_edges: BTreeSet<usize> = (0..engine.edges.len()).filter(|&e| engine.edge_faces[e].len() > 2).collect();
    for &e in &bad_edges {
        bad.insert(engine.edges[e][0]);
        bad.insert(engine.edges[e][1]);
    }
    if bad_edges.is_empty() || !fix {
        return false;
    }
    let mut remove = vec![false; w.n_cells];
    for f in 0..w.faces.len() {
        for (a, b) in ring_edges(&w.faces[f]) {
            if engine.bp[a] == NOT_BOUNDARY || engine.bp[b] == NOT_BOUNDARY {
                continue;
            }
            if engine.point_edges[engine.bp[a]].iter().any(|&e| bad_edges.contains(&e) && same_edge((engine.edges[e][0], engine.edges[e][1]), a, b)) {
                remove[w.owner[f]] = true;
                if let Some(n) = w.neighbour[f] {
                    remove[n] = true;
                }
            }
        }
    }
    rep.n_cells_removed_at_edges += remove.iter().filter(|&&r| r).count();
    w.remove_cells(&remove);
    true
}

/// `checkIrregularSurfaceConnections::checkFaceGroupsAtBndVertices`: a
/// boundary point whose boundary faces form more than one group (connected
/// over two-face edges); with `fix`, remove every cell at such a point.
fn face_groups_at_bnd_vertices(w: &mut TopoMesh, bad: &mut BTreeSet<usize>, fix: bool, rep: &mut IrregularConnectionsReport) -> bool {
    let (engine, _) = w.surface();
    let mut invalid: BTreeSet<usize> = BTreeSet::new();
    for b in 0..engine.boundary_points.len() {
        let pf = &engine.point_faces[b];
        let mut group: BTreeMap<usize, usize> = BTreeMap::new();
        let mut n_group = 0usize;
        for &f0 in pf {
            if group.contains_key(&f0) {
                continue;
            }
            group.insert(f0, n_group);
            let mut front = vec![f0];
            while let Some(fl) = front.pop() {
                for &nf in &engine.face_faces[fl] {
                    if !group.contains_key(&nf) && pf.contains(&nf) {
                        front.push(nf);
                        group.insert(nf, n_group);
                    }
                }
            }
            n_group += 1;
        }
        if n_group != 1 {
            bad.insert(engine.boundary_points[b]);
            invalid.insert(engine.boundary_points[b]);
        }
    }
    if invalid.is_empty() || !fix {
        return false;
    }
    let pc = w.point_cells();
    let mut remove = vec![false; w.n_cells];
    for &v in &invalid {
        for &c in &pc[v] {
            remove[c] = true;
        }
    }
    rep.n_cells_removed_at_vertices += remove.iter().filter(|&&r| r).count();
    w.remove_cells(&remove);
    true
}

/// cfMesh `checkIrregularSurfaceConnections::checkIrregularVertices`: the
/// irregular boundary points, without changing the mesh.
pub fn irregular_vertices(w: &TopoMesh) -> BTreeSet<usize> {
    let mut copy = w.clone();
    let mut bad = BTreeSet::new();
    let mut rep = IrregularConnectionsReport::default();
    cell_groups_at_bnd_vertices(&mut copy, &mut bad, false, &mut rep);
    edge_face_connections(&mut copy, &mut bad, false, &mut rep);
    face_groups_at_bnd_vertices(&mut copy, &mut bad, false, &mut rep);
    bad
}

/// cfMesh `checkIrregularSurfaceConnections::checkAndFixIrregularConnections`:
/// repeat until nothing changes: split points at unconnected cell groups,
/// remove cells at edges with more than two boundary faces, remove cells at
/// points with unconnected boundary-face groups; then remove unused points.
/// Upstream returns whether any bad vertex was seen (`report.bad_vertices`
/// non-empty).
pub fn check_and_fix_irregular_connections(w: &mut TopoMesh) -> IrregularConnectionsReport {
    let mut rep = IrregularConnectionsReport::default();
    let mut bad = BTreeSet::new();
    loop {
        let mut finished = true;
        while cell_groups_at_bnd_vertices(w, &mut bad, true, &mut rep) {
            finished = false;
        }
        while edge_face_connections(w, &mut bad, true, &mut rep) {
            finished = false;
        }
        if face_groups_at_bnd_vertices(w, &mut bad, true, &mut rep) {
            finished = false;
        }
        if finished {
            break;
        }
    }
    w.remove_unused_vertices();
    rep.bad_vertices = bad;
    rep
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;
    use crate::carve::carve_box;
    use crate::shapes::sphere_surface;

    /// Every cell's edges in exactly two of its faces, and the mesh closed
    /// geometrically ([`VolumeMesh::validate`]).
    fn closed(w: &TopoMesh) -> bool {
        let cf = w.cell_faces();
        cf.iter().all(|fs| w.cell_is_closed(fs)) && w.clone().into_volume_mesh().0.validate().is_ok()
    }

    fn volume(w: &TopoMesh) -> f64 {
        w.clone().into_volume_mesh().0.total_volume()
    }

    fn same(a: &TopoMesh, b: &TopoMesh) -> bool {
        a.points.len() == b.points.len()
            && a.points.iter().zip(&b.points).all(|(p, q)| p.x.to_bits() == q.x.to_bits() && p.y.to_bits() == q.y.to_bits() && p.z.to_bits() == q.z.to_bits())
            && a.faces == b.faces
            && a.owner == b.owner
            && a.neighbour == b.neighbour
            && a.region == b.region
            && a.n_cells == b.n_cells
    }

    fn pts2(xy: &[(f64, f64)]) -> Vec<Vec3> {
        xy.iter().map(|&(x, y)| Vec3::new(x, y, 0.0)).collect()
    }

    /// V&V (gh:#1267) — `faceDecomposition` by hand. An L-shaped hexagon
    /// (0,0) (2,0) (2,1) (1,1) (1,2) (0,2), counter-clockwise, normal +z.
    /// Predicted before the first run: planar; the one concave vertex is
    /// point 3, (1,1); `decomposeFaceIntoTriangles` fans from it:
    /// [3,4,5] [3,5,0] [3,0,1] [3,1,2]; `decomposeFace` merges from both
    /// ends into two convex quads [5,0,3,4] and [1,2,3,0] of area 1.5 each,
    /// wound as the hexagon. A unit square is convex and comes back whole;
    /// a face with two concave vertices is upstream's fatal error.
    ///
    /// Result (2026-11-05): as predicted.
    #[test]
    fn face_decomposition_by_hand() {
        let p = pts2(&[(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0), (1.0, 2.0), (0.0, 2.0)]);
        let l = [0, 1, 2, 3, 4, 5];
        assert!(is_face_planar(&l, &p));
        assert_eq!(concave_vertex(&l, &p), Ok(Some(3)));
        assert_eq!(decompose_face_into_triangles(&l, &p).unwrap(), vec![vec![3, 4, 5], vec![3, 5, 0], vec![3, 0, 1], vec![3, 1, 2]]);
        let parts = decompose_face(&l, &p).unwrap();
        assert_eq!(parts, vec![vec![5, 0, 3, 4], vec![1, 2, 3, 0]]);
        for q in &parts {
            assert!(is_face_convex(q, &p).unwrap());
            assert!((ring_area_vector(&p, q).z - 1.5).abs() < 1e-15);
        }
        let sq = pts2(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]);
        assert_eq!(decompose_face(&[0, 1, 2, 3], &sq).unwrap(), vec![vec![0, 1, 2, 3]]);
        // A notch with two concave vertices.
        let two = pts2(&[(0.0, 0.0), (3.0, 0.0), (3.0, 3.0), (2.0, 1.0), (1.0, 1.0), (0.0, 3.0)]);
        assert_eq!(concave_vertex(&[0, 1, 2, 3, 4, 5], &two), Err(TopologyError::MoreThanOneConcaveVertex));
        // A twisted quad is not planar at 5 %.
        let mut tw = sq.clone();
        tw[2].z = 0.5;
        assert!(!is_face_planar(&[0, 1, 2, 3], &tw));
        assert_eq!(sort_edge_chain(&[(0, 1), (2, 1)]), Err(TopologyError::ChainNotOriented));
        assert_eq!(sort_edge_chain(&[(0, 1), (1, 0), (2, 3), (3, 2)]), Err(TopologyError::OpenChain));
    }

    /// V&V (gh:#1267) — `decomposeMeshFaces`, `decomposeConcaveInternalFaces`
    /// and `removeCells` on a 2x2x2 box. Predicted before the first run:
    /// fanning the 12 internal faces gives 12 x 4 triangles about 12 new
    /// points, the volume unchanged to round-off (planar faces) and the mesh
    /// closed; one concave vertex fans a quad into 2 triangles with no new
    /// point (each internal face through that point), two on one face fan it
    /// about a new centre; removing cell 0 leaves 7 cells, volume 7/8,
    /// closed, every boundary face in the one patch `walls` (the box has
    /// exactly one patch, so it is kept); with two patches, `defaultFaces`.
    ///
    /// Result (2026-11-05): all held except the patch: **miss**, the
    /// prediction misread the input. `cartesian_box` names its six sides
    /// (six patches), so `removeCells` collapses them into `defaultFaces`,
    /// as upstream's `reorderBoundaryFaces` does. A one-patch mesh keeps
    /// its patch (checked below on the box with its sides merged).
    #[test]
    fn face_fans_and_cell_removal_on_a_box() {
        let m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [2, 2, 2]);
        let base = TopoMesh::from_volume_mesh(&m);
        let mut w = base.clone();
        let mark: Vec<bool> = w.neighbour.iter().map(|n| n.is_some()).collect();
        assert_eq!(w.decompose_mesh_faces(&mark), 12);
        assert_eq!(w.faces.len(), base.faces.len() + 12 * 3);
        assert_eq!(w.points.len(), base.points.len() + 12);
        assert!(closed(&w));
        assert!((volume(&w) - 1.0).abs() < 1e-14);

        let f0 = (0..base.faces.len()).find(|&f| base.neighbour[f].is_some()).unwrap();
        let ring = base.faces[f0].clone();
        let mut one = vec![false; base.points.len()];
        one[ring[0]] = true;
        let mut w1 = base.clone();
        let n1 = w1.decompose_concave_internal_faces(&one);
        assert!(n1 >= 1);
        assert_eq!(w1.points.len(), base.points.len());
        assert_eq!(w1.faces.len(), base.faces.len() + n1);
        assert!(closed(&w1));
        let mut two = one.clone();
        two[ring[2]] = true;
        let mut w2 = base.clone();
        w2.decompose_concave_internal_faces(&two);
        assert!(w2.points.len() > base.points.len());
        assert!(closed(&w2));
        assert!((volume(&w2) - 1.0).abs() < 1e-14);

        let mut w3 = base.clone();
        let mut rm = vec![false; 8];
        rm[0] = true;
        assert!(w3.remove_cells(&rm));
        assert_eq!(w3.n_cells, 7);
        assert_eq!(w3.patch_names, vec!["defaultFaces".to_string()]);
        let mut one_patch = base.clone();
        one_patch.patch_names = vec!["walls".into()];
        for (r, n) in one_patch.region.iter_mut().zip(&one_patch.neighbour) {
            *r = n.is_none().then_some(0);
        }
        assert!(!one_patch.remove_cells(&rm));
        assert_eq!(one_patch.patch_names, vec!["walls".to_string()]);
        assert!(closed(&w3));
        assert!((volume(&w3) - 0.875).abs() < 1e-14);
    }

    /// Draw the cut of `m` by the plane `x[axis] = at` as a PNG (a PPM
    /// raster converted by ImageMagick `convert`, which adds the caption):
    /// each face's crossing segment, internal faces grey, boundary faces red
    /// (or `colour(face)` when it gives one), plus reference circles
    /// `(centre_u, centre_v, r)` dashed blue, in the window
    /// `[u0, u1] x [v0, v1]` of the other two axes (in axis order). Used by
    /// the `--ignored` image tests (gh:#1267, #1270, #1271).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn write_slice_png(
        m: &VolumeMesh,
        axis: usize,
        at: f64,
        window: [f64; 4],
        circles: &[(f64, f64, f64)],
        colour: &dyn Fn(usize) -> Option<[u8; 3]>,
        title: &str,
        path: &std::path::Path,
    ) {
        let (ua, va) = match axis {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let c = |p: Vec3, i: usize| [p.x, p.y, p.z][i];
        let [u0, u1, v0, v1] = window;
        let s = 1000.0 / (u1 - u0).max(v1 - v0);
        let (wd, ht) = (((u1 - u0) * s).ceil() as usize, ((v1 - v0) * s).ceil() as usize);
        let mut px = vec![[255u8; 3]; wd * ht];
        let mut dot = |x: f64, y: f64, col: [u8; 3], r: i64| {
            let (xi, yi) = (x.round() as i64, y.round() as i64);
            for dy in -r..=r {
                for dx in -r..=r {
                    let (a, b) = (xi + dx, yi + dy);
                    if a >= 0 && b >= 0 && (a as usize) < wd && (b as usize) < ht {
                        px[b as usize * wd + a as usize] = col;
                    }
                }
            }
        };
        let tx = |u: f64| (u - u0) * s;
        let ty = |v: f64| ht as f64 - (v - v0) * s;
        let mut segs: Vec<(bool, [u8; 3], (f64, f64), (f64, f64))> = Vec::new();
        for f in 0..m.face_count() {
            let ring = &m.faces[f];
            let k = ring.len();
            let mut hits = Vec::new();
            for i in 0..k {
                let (a, b) = (m.points[ring[i]], m.points[ring[(i + 1) % k]]);
                let (da, db) = (c(a, axis) - at, c(b, axis) - at);
                if (da < 0.0) != (db < 0.0) {
                    let t = da / (da - db);
                    let p = a.add(b.sub(a).scale(t));
                    hits.push((tx(c(p, ua)), ty(c(p, va))));
                }
            }
            let bnd = m.neighbour[f].is_none();
            let col = colour(f).unwrap_or(if bnd { [214, 39, 40] } else { [150, 150, 150] });
            for pair in hits.chunks(2) {
                if pair.len() == 2 {
                    segs.push((bnd || colour(f).is_some(), col, pair[0], pair[1]));
                }
            }
        }
        segs.sort_by_key(|s| s.0);
        for (thick, col, a, b) in segs {
            let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as usize).max(1);
            for i in 0..=n {
                let t = i as f64 / n as f64;
                dot(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, col, i64::from(thick));
            }
        }
        for &(cu, cv, r) in circles {
            let n = (2.0 * std::f64::consts::PI * r * s).ceil() as usize;
            for i in 0..n {
                if (i / 6) % 2 == 0 {
                    let th = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
                    dot(tx(cu + r * th.cos()), ty(cv + r * th.sin()), [31, 119, 180], 0);
                }
            }
        }
        let mut ppm = format!("P6\n{wd} {ht}\n255\n").into_bytes();
        for p in &px {
            ppm.extend_from_slice(p);
        }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let ppm_path = path.with_extension("ppm");
        std::fs::write(&ppm_path, ppm).unwrap();
        let names = ["x", "y", "z"];
        let caption = format!(
            "{title}\ncut {} = {at} m; window {} [{u0}, {u1}], {} [{v0}, {v1}] m; red/coloured: boundary or marked faces, grey: internal, blue dashed: reference",
            names[axis], names[ua], names[va]
        );
        let ok = std::process::Command::new("convert")
            .arg(&ppm_path)
            .args(["-background", "white", "-gravity", "south", "-splice", "0x56", "-pointsize", "15", "-annotate", "+0+6"])
            .arg(&caption)
            .arg(path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        std::fs::remove_file(&ppm_path).ok();
        assert!(ok, "ImageMagick convert failed for {}", path.display());
    }

    /// Images (gh:#1267), `--ignored`: x-z cuts at y = 0.3 m (through cell
    /// centres of the 0.6 m carve) of the carved sphere, after
    /// `surfaceMorpherCells`, and after `topologicalCleaner`, with the
    /// R 3 m sphere dashed; and the raw 3x3x3 box after the cleaner, cut at
    /// y = 0.5. Written to
    /// `verification_and_validation/topology_gh1267/`.
    #[test]
    #[ignore]
    fn draw_topology_images() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/topology_gh1267");
        let none = |_f: usize| None;
        let w0 = carved_sphere();
        let win = [-3.6, 3.6, -3.6, 3.6];
        let circ = [(0.0, 0.0, 3.0)];
        write_slice_png(&w0.clone().into_volume_mesh().0, 1, 0.3, win, &circ, &none, "carved sphere R 3 m, h 0.6 m", &dir.join("sphere_carved_xz.png"));
        let mut w = w0.clone();
        surface_morpher_cells(&mut w).unwrap();
        write_slice_png(&w.clone().into_volume_mesh().0, 1, 0.3, win, &circ, &none, "after surfaceMorpherCells", &dir.join("sphere_morphed_xz.png"));
        topological_cleaner(&mut w);
        write_slice_png(&w.clone().into_volume_mesh().0, 1, 0.3, win, &circ, &none, "after topologicalCleaner", &dir.join("sphere_cleaned_xz.png"));
        let mut b = TopoMesh::from_volume_mesh(&cartesian_box(Vec3::ZERO, Vec3::new(3.0, 3.0, 3.0), [3, 3, 3]));
        topological_cleaner(&mut b);
        write_slice_png(&b.into_volume_mesh().0, 1, 0.5, [-0.2, 3.2, -0.2, 3.2], &[], &none, "3x3x3 box after topologicalCleaner", &dir.join("box_cleaned_xz.png"));
    }

    /// The carved sphere of the plan (R 3 m, h 0.6 m, `carve_box`).
    pub(crate) fn carved_sphere() -> TopoMesh {
        let (p, t) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
        TopoMesh::from_volume_mesh(&carve_box(&p, &t, 0.6))
    }

    /// V&V (gh:#1267) — `surfaceMorpherCells` on the carved sphere.
    /// Methodology: R 3 m sphere (24 x 48 facets), `carve_box` at 0.6 m
    /// (a staircase, one `walls` patch), then `surface_morpher_cells`.
    /// Predicted before the first run:
    /// - the mesh is closed (topologically and geometrically) and no cell
    ///   has a non-positive volume;
    /// - no boundary cell has every vertex on the boundary;
    /// - every boundary cell has **exactly one** boundary face (a staircase
    ///   cell's exposed faces are edge-connected, so they merge);
    /// - the volume equals the sum of the kept cells' volumes (1e-12
    ///   relative), and differs from the carved volume minus the removed
    ///   cells' carved volumes by under 1 % (merged boundary faces are
    ///   non-planar, so their centroid-fan volume differs), non-zero
    ///   when any face merged;
    /// - the boundary is one patch, `defaultFaces`.
    ///
    /// Results (2026-11-05): 552 cells, none removed; 276 internal faces
    /// shrunk, 760 boundary faces merged, 3 passes; closed, every cell
    /// volume positive; 272 boundary cells, **each with exactly one boundary
    /// face** (held); the volume equals the kept cells' sum (held). **Miss:**
    /// the volume fell from 119.232 to 93.024 m³ (−22 %, predicted under
    /// 1 %). `diagnose_morpher_volume_by_step` locates it: the internal-face
    /// step conserves the volume exactly, and the boundary merge carries
    /// all of it. A staircase corner's exposed faces merge into one
    /// non-planar polygon, and a non-planar face's volume contribution is
    /// taken through its centroid (here and in OpenFOAM/cfMesh alike), so
    /// the merged face measures as a chamfer across the corner. The
    /// morphed boundary is not a surface to measure: upstream maps it onto
    /// the geometry next (`cartesianMeshGenerator::mapMeshToSurface`). The
    /// number is pinned below, not a pass criterion.
    #[test]
    fn surface_morpher_cells_on_the_carved_sphere() {
        let w0 = carved_sphere();
        let v0 = volume(&w0);
        let mut w = w0.clone();
        let rep = surface_morpher_cells(&mut w).unwrap();
        let v1 = volume(&w);
        println!("morpher: {} cells -> {}, volume {v0:.9} -> {v1:.9}; {rep:?}", w0.n_cells, w.n_cells);
        assert!(closed(&w));
        let (vols, _) = crate::checks::cell_geometry(&w.clone().into_volume_mesh().0);
        assert!(vols.iter().all(|&v| v > 0.0));
        let bv = w.boundary_vertex();
        let cf = w.cell_faces();
        let mut n_bfaces = vec![0usize; w.n_cells];
        for f in 0..w.faces.len() {
            if w.neighbour[f].is_none() {
                n_bfaces[w.owner[f]] += 1;
            }
        }
        for c in 0..w.n_cells {
            if n_bfaces[c] > 0 {
                assert!(!cf[c].iter().all(|&f| w.faces[f].iter().all(|&v| bv[v])), "cell {c} all on the boundary");
            }
        }
        let multi = n_bfaces.iter().filter(|&&n| n > 1).count();
        let kept_sum: f64 = vols.iter().sum();
        // A carved cell is a cube of side 0.6: the removed cells' volume.
        let removed_v = (w0.n_cells - w.n_cells) as f64 * 0.6f64.powi(3);
        println!(
            "boundary cells {}, {multi} with more than one boundary face; kept sum {kept_sum:.9}; carved minus removed {:.9}, residual {:.3e} relative",
            n_bfaces.iter().filter(|&&n| n > 0).count(),
            v0 - removed_v,
            (v1 - (v0 - removed_v)) / v1
        );
        assert_eq!(w.patch_names, vec!["defaultFaces".to_string()]);
        assert!(((v1 - kept_sum) / v1).abs() < 1e-12);
        assert_eq!(rep, SurfaceMorpherReport { iterations: 3, n_cells_removed: 0, n_internal_faces_morphed: 276, n_internal_faces_removed: 0, n_boundary_faces_merged: 760 });
        assert!((v1 - 93.024).abs() < 1e-9);
        assert_eq!(multi, 0);
    }

    /// Diagnostic (gh:#1267): where the morpher's volume goes. The carved
    /// sphere through each morpher step alone: the internal-face step must
    /// conserve the volume exactly (a split of planar faces); the boundary
    /// merge carries all of the change.
    #[test]
    fn diagnose_morpher_volume_by_step() {
        let w0 = carved_sphere();
        let v0 = volume(&w0);
        let mut w = w0.clone();
        let bv = w.boundary_vertex();
        let mut rep = SurfaceMorpherReport::default();
        assert!(morph_internal_faces(&mut w, &bv, &mut rep));
        let v_int = volume(&w);
        let mut bc = vec![false; w.n_cells];
        for f in 0..w.faces.len() {
            if w.neighbour[f].is_none() {
                bc[w.owner[f]] = true;
            }
        }
        morph_boundary_faces(&mut w, &bc, &mut rep).unwrap();
        let v_merge = volume(&w);
        println!("carved {v0:.9}; after morphInternalFaces {v_int:.9}; after one morphBoundaryFaces {v_merge:.9}; {rep:?}");
        assert!(((v_int - v0) / v0).abs() < 1e-12);
        assert!(closed(&w));
    }

    /// V&V (gh:#1267) — `topologicalCleaner` after the morpher on the carved
    /// sphere, and on a raw 3x3x3 box. Predicted before the first run: the
    /// mesh stays closed with every cell volume positive; the volume is
    /// conserved to 1e-6 relative on the sphere (fans and pyramids
    /// repartition, exact on planar faces; the morpher's merged faces are
    /// non-planar) and to 1e-12 on the box; on the morphed sphere
    /// `checkNonMappableCells` finds 0 cells (one boundary face each,
    /// previous test); on the raw box the 8 corner cells (3 boundary faces)
    /// and 12 edge cells (2) are non-mappable, so `changed`; one connected
    /// region remains in both.
    ///
    /// Results (2026-11-05), every prediction held. Morphed sphere: nothing
    /// to clean (`changed` false, 552 cells, volume unchanged, smallest
    /// cell 0.036 m³). Box: 20 non-mappable cells and 24 internal faces
    /// fanned (each with three boundary points), 20 cells into pyramids:
    /// 271 cells, volume 27 to 1e-12, closed, one region.
    #[test]
    fn topological_cleaner_on_the_morphed_sphere_and_a_box() {
        let mut w = carved_sphere();
        surface_morpher_cells(&mut w).unwrap();
        let v0 = volume(&w);
        let n0 = w.n_cells;
        let rep = topological_cleaner(&mut w);
        let v1 = volume(&w);
        let (vols, _) = crate::checks::cell_geometry(&w.clone().into_volume_mesh().0);
        println!(
            "cleaner on morphed sphere: {n0} -> {} cells, volume {v0:.9} -> {v1:.9} ({:.2e}), min cell volume {:.3e}; {rep:?}",
            w.n_cells,
            (v1 - v0) / v0,
            vols.iter().cloned().fold(f64::MAX, f64::min)
        );
        assert!(closed(&w));
        assert_eq!(rep.n_non_mappable_cells, 0);
        assert!(((v1 - v0) / v0).abs() < 1e-6);
        assert!(vols.iter().all(|&v| v > 0.0));
        assert_eq!(cell_groups(&w).1, 1);

        let mut b = TopoMesh::from_volume_mesh(&cartesian_box(Vec3::ZERO, Vec3::new(3.0, 3.0, 3.0), [3, 3, 3]));
        let rep = topological_cleaner(&mut b);
        println!("cleaner on 3x3x3 box: {} cells, volume {:.15}; {rep:?}", b.n_cells, volume(&b));
        assert_eq!(rep.n_non_mappable_cells, 20);
        assert!(rep.changed);
        assert_eq!((b.n_cells, rep.n_non_mappable_faces, rep.n_decomposed_cells), (271, 24, 20));
        assert!(closed(&b));
        assert!((volume(&b) - 27.0).abs() < 1e-12);
        assert_eq!(cell_groups(&b).1, 1);
    }

    /// Two unit cubes touching at one vertex, (1, 1, 1).
    fn two_cubes_at_a_vertex() -> TopoMesh {
        let a = TopoMesh::from_volume_mesh(&cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [1, 1, 1]));
        let b = TopoMesh::from_volume_mesh(&cartesian_box(Vec3::new(1.0, 1.0, 1.0), Vec3::new(2.0, 2.0, 2.0), [1, 1, 1]));
        let corner = Vec3::new(1.0, 1.0, 1.0);
        let mut w = a.clone();
        let shared_a = a.points.iter().position(|p| p.sub(corner).length() < 1e-12).unwrap();
        let off = w.points.len();
        w.points.extend(b.points.iter().copied());
        for f in 0..b.faces.len() {
            w.faces.push(b.faces[f].iter().map(|&v| if b.points[v].sub(corner).length() < 1e-12 { shared_a } else { off + v }).collect());
            w.owner.push(1);
            w.neighbour.push(None);
            w.region.push(Some(0));
        }
        w.n_cells = 2;
        w.remove_unused_vertices();
        w
    }

    /// V&V (gh:#1268) — `checkIrregularSurfaceConnections` and
    /// `checkCellConnectionsOverFaces` on two cubes touching at a vertex.
    /// Predicted before the first run: the shared point is the one
    /// irregular vertex (two cell groups and two boundary-face groups at
    /// it); the fix splits it (one point split, 16 points after, no cell
    /// removed, volume 2, closed); the cell-connection check then finds two
    /// regions of one cell each and keeps the first (the cube at the
    /// origin): 1 cell, volume 1.
    ///
    /// Result (2026-11-05): as predicted.
    #[test]
    fn irregular_vertex_between_two_cubes() {
        let mut w = two_cubes_at_a_vertex();
        assert_eq!(w.points.len(), 15);
        let shared: BTreeSet<usize> = (0..w.points.len()).filter(|&v| w.points[v].sub(Vec3::new(1.0, 1.0, 1.0)).length() < 1e-12).collect();
        assert_eq!(irregular_vertices(&w), shared);
        let rep = check_and_fix_irregular_connections(&mut w);
        println!("{rep:?}");
        assert_eq!(rep.n_points_split, 1);
        assert_eq!(rep.n_cells_removed_at_edges + rep.n_cells_removed_at_vertices, 0);
        assert_eq!(w.points.len(), 16);
        assert_eq!(w.n_cells, 2);
        assert!(closed(&w));
        assert!((volume(&w) - 2.0).abs() < 1e-14);
        assert!(irregular_vertices(&w).is_empty());
        assert!(check_cell_connections_over_faces(&mut w));
        assert_eq!(w.n_cells, 1);
        assert!((volume(&w) - 1.0).abs() < 1e-14);
        w.remove_unused_vertices();
        assert!(w.points.iter().all(|p| p.x <= 1.0 && p.y <= 1.0 && p.z <= 1.0));
    }

    /// V&V (gh:#1268) — `checkBoundaryFacesSharingTwoEdges` by hand. A unit
    /// hex whose top-front edge carries an extra point m = (0.5, 0, 1) in
    /// both the top and the front face: m is in exactly two boundary faces,
    /// which share the edges (0,0,1)–m and m–(1,0,1). Predicted before the
    /// first run: `findPoints` = {m}; m is in no internal face, so it is
    /// removable: `improveTopology` removes it (1 point) and fans nothing;
    /// the result is the plain hex, every face a quad, volume 1, closed.
    ///
    /// Result (2026-11-05): as predicted.
    #[test]
    fn boundary_faces_sharing_two_edges_by_hand() {
        let mut w = TopoMesh::from_volume_mesh(&cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [1, 1, 1]));
        let at = |w: &TopoMesh, x: f64, y: f64, z: f64| w.points.iter().position(|p| p.sub(Vec3::new(x, y, z)).length() < 1e-12).unwrap();
        let (a, b) = (at(&w, 0.0, 0.0, 1.0), at(&w, 1.0, 0.0, 1.0));
        let m = w.points.len();
        w.points.push(Vec3::new(0.5, 0.0, 1.0));
        for ring in w.faces.iter_mut() {
            let k = ring.len();
            if let Some(i) = (0..k).find(|&i| same_edge((ring[i], ring[(i + 1) % k]), a, b)) {
                ring.insert(i + 1, m);
            }
        }
        assert!(closed(&w));
        assert_eq!(BoundaryFacesSharingTwoEdges::find_points(&w), BTreeSet::from([m]));
        assert_eq!(BoundaryFacesSharingTwoEdges::improve_topology(&mut w), (1, 0));
        assert_eq!(w.points.len(), 8);
        assert!(w.faces.iter().all(|r| r.len() == 4));
        assert!(closed(&w));
        assert!((volume(&w) - 1.0).abs() < 1e-15);
        assert!(BoundaryFacesSharingTwoEdges::find_points(&w).is_empty());
    }

    /// V&V (gh:#1268) — `checkNonMappableCellConnections`. Predicted before
    /// the first run: on a 3x1x1 row every cell has all its vertices on the
    /// boundary; the middle one has two internal faces with no shared edge
    /// (`INTERNALFACEGROUP`), the ends one neighbour each, so all three go
    /// in the first pass (upstream's rule; a one-cell-thick mesh is not
    /// mappable). On a 3x3x3 box no cell has all its vertices on the
    /// boundary: nothing is removed.
    ///
    /// Result (2026-11-05): as predicted.
    #[test]
    fn non_mappable_cell_connections() {
        let mut row = TopoMesh::from_volume_mesh(&cartesian_box(Vec3::ZERO, Vec3::new(3.0, 1.0, 1.0), [3, 1, 1]));
        assert_eq!(non_mappable_cells(&row), BTreeSet::from([0, 1, 2]));
        assert_eq!(remove_non_mappable_cells(&mut row), 3);
        assert_eq!(row.n_cells, 0);
        let mut b = TopoMesh::from_volume_mesh(&cartesian_box(Vec3::ZERO, Vec3::new(3.0, 3.0, 3.0), [3, 3, 3]));
        assert_eq!(remove_non_mappable_cells(&mut b), 0);
    }

    /// V&V (gh:#1268) — the four checks leave a clean box bit for bit.
    /// Predicted before the first run: on a 3x3x3 box no irregular vertex,
    /// no non-mappable cell, no point in two boundary faces, one region:
    /// the mesh is unchanged, every bit.
    ///
    /// Result (2026-11-05): as predicted.
    #[test]
    fn clean_box_is_left_bit_identical() {
        let base = TopoMesh::from_volume_mesh(&cartesian_box(Vec3::ZERO, Vec3::new(3.0, 3.0, 3.0), [3, 3, 3]));
        let mut w = base.clone();
        let rep = check_and_fix_irregular_connections(&mut w);
        assert!(rep.bad_vertices.is_empty());
        assert_eq!(remove_non_mappable_cells(&mut w), 0);
        assert_eq!(BoundaryFacesSharingTwoEdges::improve_topology(&mut w), (0, 0));
        assert!(!check_cell_connections_over_faces(&mut w));
        assert!(same(&w, &base));
    }
}
