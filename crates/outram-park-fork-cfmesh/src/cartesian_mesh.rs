// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/octrees/meshOctree/meshOctreeAddressing/
//     meshOctreeAddressing.{C,H} (constructor, findEdgeCentre,
//       cubesAroundEdge),
//     meshOctreeAddressingCreation.C (createOctreePoints, createNodeLabels,
//       createNodeLeaves, findUsedBoxes, calculateNodeType,
//       createOctreeFaces, calculateLeafFaces)
//   meshLibrary/cartesianMesh/cartesianMeshExtractor/
//     cartesianMeshExtractor.{C,H}, cartesianMeshExtractorPointsAndAddressing.C,
//     cartesianMeshExtractorPolyMesh.C, cartesianMeshExtractorDecomposeSplitHexes.C
//   meshLibrary/utilities/faceDecomposition/decomposeFaces.C and
//   meshLibrary/utilities/decomposeCells/ (through crate::correct_edges)
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

//! cfMesh's **cartesian mesh extraction** (gh:#1266): the octree's leaves
//! become the cells of a [`VolumeMesh`], through upstream's
//! `meshOctreeAddressing` (node numbering, used boxes, octree faces with
//! hanging nodes) and `cartesianMeshExtractor` (cells, faces, optional
//! decomposition of split hexes into tetrahedra and pyramids).
//!
//! This is upstream's route beside the crate's own
//! [`crate::octree`] assembler (`build_mesh`), which is unchanged; the two
//! are compared on the same leaves in the tests.
//!
//! # Which leaves become cells
//!
//! `findUsedBoxes`: a leaf is a `MESHCELL` if it is `INSIDE` and holds no
//! triangle and no feature edge, or, with
//! [`CartesianMeshOptions::use_data_boxes`] (`keepCellsIntersectingBoundary`),
//! if it holds any. The face neighbours of `MESHCELL` leaves that are not
//! cells are `BOUNDARY` leaves: they get nodes (so the boundary faces of
//! finer neighbours can be built) but no cell.
//!
//! # The dictionary options (gh:#1266 remainder)
//!
//! [`MeshOctreeAddressing::from_dict`] and
//! [`cartesian_mesh_extractor_from_dict`] read `nonManifoldMeshing`,
//! `keepCellsIntersectingPatches`, `removeCellsIntersectingPatches`
//! ([`CellSelection`]) and `checkForGluedMesh`
//! ([`MeshOctreeAddressing::check_glued_regions`]) from a [`MeshDict`];
//! with none set they are [`MeshOctreeAddressing::new`] and
//! [`cartesian_mesh_extractor`]. The octree edges and their maps
//! ([`MeshOctreeAddressing::edge_addressing`]), `leafLeaves`, `nodeFaces`,
//! `isIntersectedFace` / `isIntersectedEdge` / `edgeIntersections` and
//! `checkAndFixIrregularConnections` (no upstream caller) are methods.
//!
//! # Not ported
//!
//! ~~`nonManifoldMeshing`, `removeCellsIntersectingPatches`,
//! `keepCellsIntersectingPatches`, `checkGluedRegions`
//! (`checkForGluedMesh`), `checkAndFixIrregularConnections`, the octree
//! edges and their addressing (`createOctreeEdges` and the five
//! edge-related maps), `isIntersectedFace` / `isIntersectedEdge`,
//! `leafLeaves`, `nodeFaces`, the quadtree's empty patches~~ (**ported
//! 2026-11-05**: the first group above, the empty patches by gh:#1275)
//! and everything parallel. All boundary faces of an octree go to one
//! patch, `defaultFaces` (upstream's `polyMeshGen` leaves them unnamed
//! until the generator assigns patches). **Quadtree (gh:#1275):** the
//! faces whose normal is z-dominated go to the empty patches
//! `unusedFacesBottom` / `unusedFacesTop` ([`create_empty_patches`]).

use crate::math::Vec3;
use crate::mesh_octree::{help, CubeType, MeshOctree, TriSurf, EDGE_NODES, FACE_EDGES, FACE_NODES, SMALL};
use crate::mesh_octree_creator::MeshDict;
use crate::volume_mesh::{BoundaryPatch, VolumeMesh};
use std::collections::{BTreeMap, BTreeSet};

/// `meshOctreeAddressing::octreeCubeTypes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxType;

impl BoxType {
    /// Not used.
    pub const NONE: u8 = 0;
    /// A mesh cell.
    pub const MESHCELL: u8 = 1;
    /// A face neighbour of a mesh cell that is not one.
    pub const BOUNDARY: u8 = 2;
    /// A split hex (not set by the serial routines ported here).
    pub const SPLITHEX: u8 = 4;
    /// Node next to an `INSIDE` leaf without triangles.
    pub const INNERNODE: u8 = 8;
    /// Node next to an `OUTSIDE` or `UNKNOWN` leaf.
    pub const OUTERNODE: u8 = 16;
    /// Boundary node (not set by the serial routines ported here).
    pub const BOUNDARYNODE: u8 = 32;
}

/// `meshOctreeAddressing`: nodes and faces of the used octree leaves.
#[derive(Debug, Clone)]
pub struct MeshOctreeAddressing {
    /// `useDATABoxes_`.
    pub use_data_boxes: bool,
    /// `boxType`, per leaf.
    pub box_type: Vec<u8>,
    /// `nodeLabels`: the 8 corner nodes of each used leaf (Z-order corners).
    pub node_labels: Vec<Option<[usize; 8]>>,
    /// Corners no leaf owned (upstream reports them as "bad leaves").
    pub n_unlabelled_corners: usize,
    /// `octreePoints`.
    pub octree_points: Vec<Vec3>,
    /// `nodeLeaves`: the used leaves around each node (positions as in
    /// `findLeavesForCubeVertex`).
    pub node_leaves: Vec<[Option<usize>; 8]>,
    /// `nodeType`.
    pub node_type: Vec<u8>,
    /// `octreeFaces`.
    pub octree_faces: Vec<Vec<usize>>,
    /// `octreeFaceOwner` (leaf labels).
    pub octree_face_owner: Vec<usize>,
    /// `octreeFaceNeighbour` (leaf labels; `None` at the root's boundary).
    pub octree_face_neighbour: Vec<Option<usize>>,
    /// `leafFaces`: the octree faces of each leaf.
    pub leaf_faces: Vec<Vec<usize>>,
}

impl MeshOctreeAddressing {
    /// `meshOctreeAddressing(octree, dict, useDATABoxes)`, with every
    /// demand-driven member computed: used boxes, node labels and points,
    /// node leaves and types, octree faces and leaf faces.
    pub fn new(o: &MeshOctree, use_data_boxes: bool) -> Self {
        Self::from_box_type(o, use_data_boxes, find_used_boxes(o, use_data_boxes, &CellSelection::default()))
    }

    /// `meshOctreeAddressing(octree, meshDict, useDATABoxes)` with the
    /// dictionary entries it reads: `keepCellsIntersectingBoundary` and
    /// `nonManifoldMeshing` turn DATA boxes on; `nonManifoldMeshing`,
    /// `keepCellsIntersectingPatches` and `removeCellsIntersectingPatches`
    /// change `findUsedBoxes` ([`CellSelection`]); `checkForGluedMesh` runs
    /// [`MeshOctreeAddressing::check_glued_regions`] at the end of the
    /// constructor, as upstream. With a dict that sets none of them this is
    /// [`MeshOctreeAddressing::new`].
    pub fn from_dict(o: &MeshOctree, dict: &MeshDict, use_data_boxes: bool) -> Self {
        let sel = CellSelection::from_dict(dict);
        let use_data_boxes = use_data_boxes || dict.keep_cells_intersecting_boundary || sel.non_manifold_meshing;
        let mut a = Self::from_box_type(o, use_data_boxes, find_used_boxes(o, use_data_boxes, &sel));
        if use_data_boxes && dict.check_for_glued_mesh {
            a.check_glued_regions(o);
        }
        a
    }

    /// Everything after `findUsedBoxes`, from the given box types.
    fn from_box_type(o: &MeshOctree, use_data_boxes: bool, box_type: Vec<u8>) -> Self {
        let n = o.number_of_leaves();
        let mut a = MeshOctreeAddressing {
            use_data_boxes,
            box_type,
            node_labels: vec![None; n],
            n_unlabelled_corners: 0,
            octree_points: Vec::new(),
            node_leaves: Vec::new(),
            node_type: Vec::new(),
            octree_faces: Vec::new(),
            octree_face_owner: Vec::new(),
            octree_face_neighbour: Vec::new(),
            leaf_faces: Vec::new(),
        };
        a.create_node_labels(o);
        a.create_octree_points(o);
        a.create_node_leaves(o);
        a.calculate_node_type(o);
        a.create_octree_faces(o);
        a.calculate_leaf_faces(n);
        a
    }

    /// `numberOfNodes`.
    pub fn number_of_nodes(&self) -> usize {
        self.octree_points.len()
    }

    /// The used leaves among `findLeavesForCubeVertex(leaf, node)`, with
    /// upstream's validity rule: a leaf appearing at more than one of the 8
    /// positions does not have the node as a corner.
    fn valid_leaves(&self, o: &MeshOctree, leaf: usize, node: usize) -> ([Option<usize>; 8], [bool; 8], usize) {
        let mut p = o.find_leaves_for_cube_vertex(leaf, node);
        let mut valid = [true; 8];
        let mut min_leaf = leaf;
        for i in 0..8 {
            match p[i] {
                Some(l) => {
                    for j in i + 1..8 {
                        if p[j] == Some(l) {
                            valid[i] = false;
                            valid[j] = false;
                        }
                    }
                    if self.box_type[l] == BoxType::NONE {
                        valid[i] = false;
                        p[i] = None;
                    }
                    if valid[i] {
                        min_leaf = min_leaf.min(l);
                    }
                }
                None => valid[i] = false,
            }
        }
        (p, valid, min_leaf)
    }

    /// `createNodeLabels`: each corner node is numbered by the lowest leaf
    /// it is a corner of, in leaf order, and stored in every leaf it is a
    /// corner of. (Upstream's two passes, counting then numbering, give the
    /// same numbers in serial; one pass is done here.)
    fn create_node_labels(&mut self, o: &MeshOctree) {
        let n = o.number_of_leaves();
        let mut labels: Vec<[i64; 8]> = vec![[-1; 8]; n];
        let mut next = 0i64;
        for leaf in 0..n {
            if self.box_type[leaf] == BoxType::NONE {
                continue;
            }
            for node in 0..8 {
                if labels[leaf][node] >= 0 {
                    continue;
                }
                let (p, valid, min_leaf) = self.valid_leaves(o, leaf, node);
                if min_leaf == leaf && valid[7 - node] {
                    for i in 0..8 {
                        if valid[i] {
                            labels[p[i].expect("valid leaf")][7 - i] = next;
                        }
                    }
                    next += 1;
                }
            }
        }
        for leaf in 0..n {
            if self.box_type[leaf] == BoxType::NONE {
                continue;
            }
            let mut row = [0usize; 8];
            for node in 0..8 {
                if labels[leaf][node] < 0 {
                    self.n_unlabelled_corners += 1;
                    row[node] = usize::MAX;
                } else {
                    row[node] = labels[leaf][node] as usize;
                }
            }
            self.node_labels[leaf] = Some(row);
        }
        self.octree_points = vec![Vec3::ZERO; next as usize];
    }

    /// `createOctreePoints`: the corners of each used leaf
    /// (`meshOctreeCubeCoordinates::vertices`, inflated by `SMALL x` root
    /// span as upstream), the last leaf writing a node winning.
    fn create_octree_points(&mut self, o: &MeshOctree) {
        for (leaf, row) in self.node_labels.iter().enumerate() {
            if let Some(row) = row {
                let v = o.return_leaf(leaf).coords.vertices(&o.root_box);
                for (k, &node) in row.iter().enumerate() {
                    if node != usize::MAX {
                        self.octree_points[node] = v[k];
                    }
                }
            }
        }
    }

    /// `createNodeLeaves`.
    fn create_node_leaves(&mut self, o: &MeshOctree) {
        let nn = self.number_of_nodes();
        self.node_leaves = vec![[None; 8]; nn];
        let mut stored = vec![false; nn];
        for leaf in 0..self.node_labels.len() {
            let Some(row) = self.node_labels[leaf] else {
                continue;
            };
            for (k, &node) in row.iter().enumerate() {
                if node == usize::MAX || stored[node] {
                    continue;
                }
                stored[node] = true;
                let mut p = o.find_leaves_for_cube_vertex(leaf, k);
                for x in p.iter_mut() {
                    if x.is_some_and(|l| self.box_type[l] == BoxType::NONE) {
                        *x = None;
                    }
                }
                self.node_leaves[node] = p;
            }
        }
    }

    /// `calculateNodeType`.
    fn calculate_node_type(&mut self, o: &MeshOctree) {
        self.node_type = vec![BoxType::NONE; self.number_of_nodes()];
        for (node, leaves) in self.node_leaves.iter().enumerate() {
            for l in leaves.iter().flatten() {
                let t = o.return_leaf(*l).cube_type;
                if t & (CubeType::OUTSIDE | CubeType::UNKNOWN) != 0 {
                    self.node_type[node] |= BoxType::OUTERNODE;
                    break;
                } else if !o.has_contained_triangles(*l) && t & CubeType::INSIDE != 0 {
                    self.node_type[node] |= BoxType::INNERNODE;
                    break;
                }
            }
        }
    }

    /// `findEdgeCentre`: the node at the midpoint of edge `e` of `leaf`, if
    /// a finer used leaf has one there.
    pub fn find_edge_centre(&self, o: &MeshOctree, leaf: usize, e: usize) -> Option<usize> {
        if o.is_quadtree && e >= 8 {
            return None;
        }
        let row = self.node_labels[leaf]?;
        let node = row[EDGE_NODES[e][0]];
        if node == usize::MAX {
            return None;
        }
        let level = o.return_leaf(leaf).coords.level;
        let f = match e {
            0..=3 => 1,
            4..=7 => 3,
            _ => 5,
        };
        for i in 0..4 {
            let Some(lj) = self.node_leaves[node][FACE_NODES[f][i]] else {
                continue;
            };
            if o.return_leaf(lj).coords.level > level {
                let shift = (i + 2) % 4;
                let r = self.node_labels[lj]?;
                let c = r[FACE_NODES[f][shift]];
                return if c == usize::MAX { None } else { Some(c) };
            }
        }
        None
    }

    /// `cubesAroundEdge`: the 4 used leaves around edge `e` of `leaf` (from
    /// its first node's leaves).
    pub fn cubes_around_edge(&self, leaf: usize, e: usize) -> Option<[Option<usize>; 4]> {
        let node = self.node_labels[leaf]?[EDGE_NODES[e][0]];
        let p = self.node_leaves.get(node)?;
        Some(match e {
            0..=3 => [p[1], p[3], p[5], p[7]],
            4..=7 => [p[2], p[3], p[6], p[7]],
            _ => [p[4], p[5], p[6], p[7]],
        })
    }

    /// `createOctreeFaces` (module docs; upstream's serial chunking is one
    /// chunk).
    fn create_octree_faces(&mut self, o: &MeshOctree) {
        let n = o.number_of_leaves();
        for leaf in 0..n {
            let bt = self.box_type[leaf];
            let lv = o.return_leaf(leaf).coords.level;
            let cc = o.return_leaf(leaf).coords;
            if bt & BoxType::MESHCELL != 0 {
                let row = self.node_labels[leaf].expect("used leaf");
                let centres: Vec<Option<usize>> = (0..12).map(|e| self.find_edge_centre(o, leaf, e)).collect();
                for fi in 0..6 {
                    let mut neis = Vec::new();
                    o.find_neighbours_in_direction(&cc, fi, &mut neis);
                    if neis.len() != 1 {
                        continue;
                    }
                    let mut f = Vec::with_capacity(8);
                    for p in 0..4 {
                        f.push(row[FACE_NODES[fi][p]]);
                        if let Some(c) = centres[FACE_EDGES[fi][p]] {
                            f.push(c);
                        }
                    }
                    match neis[0] {
                        None => self.push_face(f, leaf, None),
                        Some(nei) if self.box_type[nei] & BoxType::MESHCELL != 0 => {
                            if nei > leaf {
                                self.push_face(f, leaf, Some(nei));
                            } else if o.return_leaf(nei).coords.level < lv {
                                // reversed: keep the first point, reverse the rest
                                f[1..].reverse();
                                self.push_face(f, nei, Some(leaf));
                            }
                        }
                        Some(nei) if self.box_type[nei] & BoxType::BOUNDARY != 0 => {
                            self.push_face(f, leaf, Some(nei));
                        }
                        Some(_) => {}
                    }
                }
            } else if bt & BoxType::BOUNDARY != 0 {
                let row = self.node_labels[leaf].expect("used leaf");
                for fi in 0..6 {
                    let mut neis = Vec::new();
                    o.find_neighbours_in_direction(&cc, fi, &mut neis);
                    if neis.len() != 1 {
                        continue;
                    }
                    let Some(nei) = neis[0] else {
                        continue;
                    };
                    if self.box_type[nei] & BoxType::MESHCELL != 0 && o.return_leaf(nei).coords.level < lv {
                        // face::reverseFace: keep the first point, reverse the rest
                        let mut f: Vec<usize> = (0..4).map(|i| row[FACE_NODES[fi][i]]).collect();
                        f[1..].reverse();
                        self.push_face(f, nei, Some(leaf));
                    }
                }
            }
        }
    }

    fn push_face(&mut self, f: Vec<usize>, own: usize, nei: Option<usize>) {
        self.octree_faces.push(f);
        self.octree_face_owner.push(own);
        self.octree_face_neighbour.push(nei);
    }

    /// `calculateLeafFaces`.
    fn calculate_leaf_faces(&mut self, n: usize) {
        self.leaf_faces = vec![Vec::new(); n];
        for f in 0..self.octree_faces.len() {
            self.leaf_faces[self.octree_face_owner[f]].push(f);
            if let Some(nb) = self.octree_face_neighbour[f] {
                self.leaf_faces[nb].push(f);
            }
        }
    }

    /// `calculateNodeFaces`: the octree faces at each node, ascending
    /// (`reverseAddressing` of `octreeFaces`).
    pub fn node_faces(&self) -> Vec<Vec<usize>> {
        let mut nf = vec![Vec::new(); self.number_of_nodes()];
        for (f, ring) in self.octree_faces.iter().enumerate() {
            for &p in ring {
                nf[p].push(f);
            }
        }
        nf
    }

    /// `calculateLeafLeaves`: the leaves across each leaf's internal octree
    /// faces, in face order (a leaf appears once per shared face).
    pub fn leaf_leaves(&self) -> Vec<Vec<usize>> {
        let mut ll = vec![Vec::new(); self.leaf_faces.len()];
        for f in 0..self.octree_faces.len() {
            if let Some(nei) = self.octree_face_neighbour[f] {
                let own = self.octree_face_owner[f];
                ll[own].push(nei);
                ll[nei].push(own);
            }
        }
        ll
    }

    /// `createOctreeEdges`, `calculateEdgeFaces`, `calculateEdgeLeaves` and
    /// `calculateLeafEdges` ([`OctreeEdges`]).
    pub fn edge_addressing(&self) -> OctreeEdges {
        let nn = self.number_of_nodes();
        let mut edges: Vec<[usize; 2]> = Vec::new();
        let mut node_edges: Vec<Vec<usize>> = vec![Vec::new(); nn];
        let mut face_edges: Vec<Vec<usize>> = Vec::with_capacity(self.octree_faces.len());
        for ring in &self.octree_faces {
            let k = ring.len();
            let mut fe = Vec::with_capacity(k);
            for i in 0..k {
                let (s, e) = (ring[i], ring[(i + 1) % k]);
                // OpenFOAM's `edge ==` ignores the direction.
                let found = node_edges[s].iter().copied().find(|&j| {
                    let ed = edges[j];
                    (ed[0] == s && ed[1] == e) || (ed[0] == e && ed[1] == s)
                });
                match found {
                    Some(j) => fe.push(j),
                    None => {
                        fe.push(edges.len());
                        node_edges[s].push(edges.len());
                        node_edges[e].push(edges.len());
                        edges.push([s, e]);
                    }
                }
            }
            face_edges.push(fe);
        }
        let mut edge_faces = vec![Vec::new(); edges.len()];
        for (f, fe) in face_edges.iter().enumerate() {
            for &e in fe {
                edge_faces[e].push(f);
            }
        }
        let mut edge_leaves: Vec<Vec<usize>> = vec![Vec::new(); edges.len()];
        for (e, ef) in edge_faces.iter().enumerate() {
            for &f in ef {
                let own = self.octree_face_owner[f];
                if !edge_leaves[e].contains(&own) {
                    edge_leaves[e].push(own);
                }
                if let Some(nei) = self.octree_face_neighbour[f] {
                    if !edge_leaves[e].contains(&nei) {
                        edge_leaves[e].push(nei);
                    }
                }
            }
        }
        let mut leaf_edges = vec![Vec::new(); self.leaf_faces.len()];
        for (e, el) in edge_leaves.iter().enumerate() {
            for &l in el {
                leaf_edges[l].push(e);
            }
        }
        OctreeEdges { edges, face_edges, node_edges, edge_faces, edge_leaves, leaf_edges }
    }

    /// Triangles appearing in every one of `leaves` (`nAppearances` equal to
    /// the number of leaves), ascending. Empty if a leaf has none.
    fn shared_triangles(o: &MeshOctree, leaves: &[usize]) -> Vec<usize> {
        let mut count: BTreeMap<usize, usize> = BTreeMap::new();
        for &l in leaves {
            for t in o.contained_triangles(l) {
                *count.entry(t).or_insert(0) += 1;
            }
        }
        count.into_iter().filter(|&(_, c)| c == leaves.len()).map(|(t, _)| t).collect()
    }

    /// `isIntersectedFace`: an internal octree face is crossed by the
    /// surface if a triangle is in both its leaves and the leaves are at the
    /// same level, or the triangle meets the face geometrically
    /// (`help::doFaceAndTriangleIntersect`).
    pub fn is_intersected_face(&self, o: &MeshOctree, f: usize) -> bool {
        let Some(nei) = self.octree_face_neighbour[f] else {
            return false;
        };
        let own = self.octree_face_owner[f];
        for t in Self::shared_triangles(o, &[own, nei]) {
            if o.return_leaf(own).coords.level == o.return_leaf(nei).coords.level {
                return true;
            }
            if do_face_and_triangle_intersect(&o.surface, t, &self.octree_faces[f], &self.octree_points) {
                return true;
            }
        }
        false
    }

    /// `isIntersectedEdge`: every leaf at the edge holds triangles, and one
    /// triangle is in all of them, with the leaves at one level or the
    /// triangle crossing the edge (`triLineIntersection`).
    pub fn is_intersected_edge(&self, o: &MeshOctree, ea: &OctreeEdges, e: usize) -> bool {
        let leaves = &ea.edge_leaves[e];
        if leaves.iter().any(|&l| !o.has_contained_triangles(l)) {
            return false;
        }
        let l0 = o.return_leaf(leaves[0]).coords.level;
        let same_level = leaves.iter().all(|&l| o.return_leaf(l).coords.level == l0);
        let [s, en] = ea.edges[e];
        for t in Self::shared_triangles(o, leaves) {
            if same_level {
                return true;
            }
            let tr = o.surface.triangles[t];
            let p = &o.surface.points;
            if help::tri_line_intersection(p[tr[0]], p[tr[1]], p[tr[2]], self.octree_points[s], self.octree_points[en]).is_some() {
                return true;
            }
        }
        false
    }

    /// `edgeIntersections`: the distinct points where the triangles shared
    /// by every leaf at edge `e` cross it (tolerance `SMALL x` its length).
    pub fn edge_intersections(&self, o: &MeshOctree, ea: &OctreeEdges, e: usize) -> Vec<Vec3> {
        let leaves = &ea.edge_leaves[e];
        if leaves.iter().any(|&l| !o.has_contained_triangles(l)) {
            return Vec::new();
        }
        let [s, en] = ea.edges[e];
        let (ps, pe) = (self.octree_points[s], self.octree_points[en]);
        let tol = SMALL * ps.sub(pe).length();
        let mut out: Vec<Vec3> = Vec::new();
        for t in Self::shared_triangles(o, leaves) {
            let tr = o.surface.triangles[t];
            let p = &o.surface.points;
            if let Some(x) = help::tri_line_intersection(p[tr[0]], p[tr[1]], p[tr[2]], ps, pe) {
                if out.iter().all(|y| y.sub(x).length() > tol) {
                    out.push(x);
                }
            }
        }
        out
    }

    /// `checkGluedRegions` (`checkForGluedMesh`): a `MESHCELL` leaf holding
    /// triangles keeps its cell only if, walking its edges from one of its
    /// `INNERNODE` corners over edges with an `INSIDE` leaf without
    /// triangles, the inner corners reached border every `MESHCELL` face
    /// neighbour; otherwise it becomes `BOUNDARY`. Returns the leaves
    /// changed.
    ///
    /// As upstream, the node and face addressing is **not** rebuilt after
    /// the change (upstream computes it inside the check and never clears
    /// it): the extractor then reads faces built with the old types, and a
    /// removed leaf's faces to `MESHCELL` neighbours become their boundary
    /// faces.
    pub fn check_glued_regions(&mut self, o: &MeshOctree) -> Vec<usize> {
        let ea = self.edge_addressing();
        let mut remove: Vec<usize> = Vec::new();
        for leaf in 0..self.box_type.len() {
            if !(o.has_contained_triangles(leaf) && self.box_type[leaf] & BoxType::MESHCELL != 0) {
                continue;
            }
            let row = self.node_labels[leaf].expect("used leaf");
            let mut inner: BTreeSet<usize> = BTreeSet::new();
            if let Some(&n0) = row.iter().find(|&&n| n != usize::MAX && self.node_type[n] & BoxType::INNERNODE != 0) {
                inner.insert(n0);
            }
            loop {
                let mut finished = true;
                for &e in &ea.leaf_edges[leaf] {
                    let [s, en] = ea.edges[e];
                    let both_inner = self.node_type[s] & BoxType::INNERNODE != 0 && self.node_type[en] & BoxType::INNERNODE != 0;
                    if !both_inner || inner.contains(&s) == inner.contains(&en) {
                        continue;
                    }
                    let found_inside = ea.edge_leaves[e]
                        .iter()
                        .any(|&l| !o.has_contained_triangles(l) && o.return_leaf(l).cube_type & CubeType::INSIDE != 0);
                    if found_inside {
                        inner.insert(s);
                        inner.insert(en);
                        finished = false;
                    }
                }
                if finished {
                    break;
                }
            }
            let permissible: BTreeSet<usize> = inner.iter().flat_map(|&n| self.node_leaves[n].iter().flatten().copied()).collect();
            if inner.is_empty() {
                remove.push(leaf);
                continue;
            }
            let cc = o.return_leaf(leaf).coords;
            for dir in 0..6 {
                let mut neis = Vec::new();
                o.find_neighbours_in_direction(&cc, dir, &mut neis);
                if neis.iter().flatten().any(|&nei| self.box_type[nei] & BoxType::MESHCELL != 0 && !permissible.contains(&nei)) {
                    remove.push(leaf);
                }
            }
        }
        remove.dedup();
        for &l in &remove {
            self.box_type[l] = BoxType::BOUNDARY;
        }
        remove
    }

    /// `checkAndFixIrregularConnections` (no caller upstream at this
    /// commit): leaves at an octree edge shared by more than two
    /// `MESHCELL`–`BOUNDARY` faces become `BOUNDARY`, as do the cells at a
    /// node where the boundary faces form two groups, until none is found;
    /// then the node and face addressing are rebuilt from the box types.
    /// Returns, per pass, the irregular (edges, nodes) found (the last is
    /// 0).
    ///
    /// **Upstream defect, transcribed:** the node test marks a node only if
    /// the walk over its boundary faces found, for **every** edge of every
    /// face, another boundary face at the node. A face's edges away from
    /// the node never have one, so the test never fires (`watertightSurface`
    /// is always false once a node has a boundary face).
    pub fn check_and_fix_irregular_connections(&mut self, o: &MeshOctree) -> Vec<(usize, usize)> {
        let ea = self.edge_addressing();
        let node_faces = self.node_faces();
        let n_faces = self.octree_faces.len();
        let mut passes = Vec::new();
        loop {
            let (mut n_edge_irr, mut n_node_irr) = (0usize, 0usize);
            let mut boundary_face = vec![false; n_faces];
            for (f, bf) in boundary_face.iter_mut().enumerate() {
                let Some(nei) = self.octree_face_neighbour[f] else {
                    continue;
                };
                let own = self.octree_face_owner[f];
                let (bo, bn) = (self.box_type[own], self.box_type[nei]);
                if (bn & BoxType::BOUNDARY != 0 && bo & BoxType::MESHCELL != 0) || (bo & BoxType::BOUNDARY != 0 && bn & BoxType::MESHCELL != 0) {
                    *bf = true;
                }
            }
            for (e, ef) in ea.edge_faces.iter().enumerate() {
                if ef.iter().filter(|&&f| boundary_face[f]).count() > 2 {
                    n_edge_irr += 1;
                    for &l in &ea.edge_leaves[e] {
                        self.box_type[l] = BoxType::BOUNDARY;
                    }
                }
            }
            for nf in &node_faces {
                let mut at: BTreeSet<usize> = nf.iter().copied().filter(|&f| boundary_face[f]).collect();
                let mut n_groups = 0;
                let mut watertight = true;
                while let Some(&first) = at.iter().next() {
                    at.remove(&first);
                    let mut front = vec![first];
                    while let Some(fl) = front.pop() {
                        for &e in &ea.face_edges[fl] {
                            let mut found = false;
                            for &fj in &ea.edge_faces[e] {
                                if at.remove(&fj) {
                                    found = true;
                                    front.push(fj);
                                }
                            }
                            if !found {
                                watertight = false;
                                break;
                            }
                        }
                    }
                    n_groups += 1;
                }
                if watertight && n_groups > 1 {
                    n_node_irr += 1;
                    for &f in nf {
                        if !boundary_face[f] {
                            continue;
                        }
                        let own = self.octree_face_owner[f];
                        if self.box_type[own] & BoxType::MESHCELL != 0 {
                            self.box_type[own] = BoxType::BOUNDARY;
                        }
                        if let Some(nei) = self.octree_face_neighbour[f] {
                            if self.box_type[nei] & BoxType::MESHCELL != 0 {
                                self.box_type[nei] = BoxType::BOUNDARY;
                            }
                        }
                    }
                }
            }
            passes.push((n_edge_irr, n_node_irr));
            if n_edge_irr + n_node_irr == 0 {
                break;
            }
        }
        // clearNodeAddressing, clearOctreeFaces, clearAddressing: rebuilt
        // from the box types on next use (here, at once).
        let rebuilt = Self::from_box_type(o, self.use_data_boxes, self.box_type.clone());
        *self = rebuilt;
        passes
    }
}

/// `meshOctreeAddressing`'s edge addressing: `octreeEdges` and its five maps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OctreeEdges {
    /// `octreeEdges`: start and end node, in the direction first met.
    pub edges: Vec<[usize; 2]>,
    /// `faceEdges`: edge `i` of a face joins its points `i` and `i + 1`.
    pub face_edges: Vec<Vec<usize>>,
    /// `nodeEdges`.
    pub node_edges: Vec<Vec<usize>>,
    /// `edgeFaces`, ascending.
    pub edge_faces: Vec<Vec<usize>>,
    /// `edgeLeaves`: owners and neighbours of the edge's faces, first-seen
    /// order.
    pub edge_leaves: Vec<Vec<usize>>,
    /// `leafEdges`, ascending.
    pub leaf_edges: Vec<Vec<usize>>,
}

/// `help::doFaceAndTriangleIntersect`: does a triangle edge cross the face
/// (fanned about OpenFOAM's `face::centre`), or a face edge cross the
/// triangle?
pub fn do_face_and_triangle_intersect(surf: &TriSurf, t: usize, ring: &[usize], points: &[Vec3]) -> bool {
    let c = crate::correct_edges::foam_face_centre(points, ring);
    let tr = surf.triangles[t];
    let tp = [surf.points[tr[0]], surf.points[tr[1]], surf.points[tr[2]]];
    let k = ring.len();
    for e in 0..3 {
        let (s, en) = (tp[e], tp[(e + 1) % 3]);
        for i in 0..k {
            if help::tri_line_intersection(points[ring[i]], points[ring[(i + 1) % k]], c, s, en).is_some() {
                return true;
            }
        }
    }
    for i in 0..k {
        if help::tri_line_intersection(tp[0], tp[1], tp[2], points[ring[i]], points[ring[(i + 1) % k]]).is_some() {
            return true;
        }
    }
    false
}

/// The `meshDict` entries `findUsedBoxes` reads besides
/// `keepCellsIntersectingBoundary` (gh:#1266 remainder). Patches are named
/// by name; a name also matches a facet subset of that name, as upstream
/// (`findPatches` is a regular expression upstream, an exact match here).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CellSelection {
    /// `nonManifoldMeshing`: `UNKNOWN` leaves become cells, and DATA boxes
    /// are used.
    pub non_manifold_meshing: bool,
    /// `keepCellsIntersectingPatches` (read only without DATA boxes).
    pub keep_cells_intersecting_patches: Vec<String>,
    /// `removeCellsIntersectingPatches` (read only with DATA boxes).
    pub remove_cells_intersecting_patches: Vec<String>,
}

impl CellSelection {
    /// The entries a [`MeshDict`] sets.
    pub fn from_dict(dict: &MeshDict) -> Self {
        CellSelection {
            non_manifold_meshing: dict.non_manifold_meshing,
            keep_cells_intersecting_patches: dict.keep_cells_intersecting_patches.clone(),
            remove_cells_intersecting_patches: dict.remove_cells_intersecting_patches.clone(),
        }
    }
}

/// The facets named by `names`: every facet of a patch called so, and every
/// facet of a facet subset called so.
fn facets_named(o: &MeshOctree, names: &[String]) -> Vec<bool> {
    let ts = &o.surface;
    let mut flag = vec![false; ts.size()];
    for name in names {
        let active: Vec<usize> = ts.find_patches(name);
        for (t, r) in ts.regions.iter().enumerate() {
            if active.contains(r) {
                flag[t] = true;
            }
        }
    }
    for name in names {
        if let Some(id) = ts.facet_subsets.index(name) {
            for t in ts.facet_subsets.elements(id) {
                flag[t] = true;
            }
        }
    }
    flag
}

/// `findUsedBoxes` (module docs; [`CellSelection`] for the options).
fn find_used_boxes(o: &MeshOctree, use_data_boxes: bool, sel: &CellSelection) -> Vec<u8> {
    let n = o.number_of_leaves();
    let mut bt = vec![BoxType::NONE; n];
    for (l, b) in bt.iter_mut().enumerate() {
        let leaf = o.return_leaf(l);
        if !o.has_contained_triangles(l) && !o.has_contained_edges(l) && leaf.cube_type & CubeType::INSIDE != 0 {
            *b |= BoxType::MESHCELL;
        }
    }
    if sel.non_manifold_meshing {
        for (l, b) in bt.iter_mut().enumerate() {
            if o.return_leaf(l).cube_type & CubeType::UNKNOWN != 0 {
                *b |= BoxType::MESHCELL;
            }
        }
    }
    if use_data_boxes {
        for (l, b) in bt.iter_mut().enumerate() {
            if o.has_contained_triangles(l) || o.has_contained_edges(l) {
                *b |= BoxType::MESHCELL;
            }
        }
        if !sel.remove_cells_intersecting_patches.is_empty() {
            let remove = facets_named(o, &sel.remove_cells_intersecting_patches);
            for (l, b) in bt.iter_mut().enumerate() {
                if o.contained_triangles(l).iter().any(|&t| remove[t]) {
                    *b = BoxType::NONE;
                }
            }
        }
    } else if !sel.keep_cells_intersecting_patches.is_empty() {
        let keep = facets_named(o, &sel.keep_cells_intersecting_patches);
        for (l, b) in bt.iter_mut().enumerate() {
            if o.contained_triangles(l).iter().any(|&t| keep[t]) {
                *b = BoxType::MESHCELL;
            }
        }
    }
    for l in 0..n {
        if bt[l] & BoxType::MESHCELL == 0 {
            continue;
        }
        let cc = o.return_leaf(l).coords;
        for nei in o.find_neighbours_for_leaf(&cc).into_iter().flatten() {
            if bt[nei] & BoxType::MESHCELL == 0 {
                bt[nei] = BoxType::BOUNDARY;
            }
        }
    }
    bt
}

/// Options of [`cartesian_mesh_extractor`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CartesianMeshOptions {
    /// `decomposeSplitHexes()`: split every non-hex cell into tetrahedra
    /// and pyramids (faces with more than 4 points fanned first).
    pub decompose_split_hexes: bool,
    /// `useDATABoxes` / `keepCellsIntersectingBoundary`.
    pub use_data_boxes: bool,
}

impl CartesianMeshOptions {
    /// The options a [`MeshDict`] sets (`keepCellsIntersectingBoundary`).
    pub fn from_dict(dict: &MeshDict) -> Self {
        CartesianMeshOptions { decompose_split_hexes: false, use_data_boxes: dict.keep_cells_intersecting_boundary }
    }
}

/// The result of [`cartesian_mesh_extractor`].
#[derive(Debug, Clone)]
pub struct CartesianMesh {
    /// The mesh; all boundary faces in patch `defaultFaces`.
    pub mesh: VolumeMesh,
    /// `leafCellLabel`: the cell of each leaf (before any decomposition).
    pub leaf_cell_label: Vec<Option<usize>>,
    /// The addressing it was built from.
    pub addressing: MeshOctreeAddressing,
    /// Cells decomposed into tetrahedra and pyramids.
    pub n_decomposed_cells: usize,
}

/// The quadtree branch of `cartesianMeshExtractor::createPolyMesh`
/// (gh:#1275): every boundary face whose area vector is z-dominated, by
/// upstream's test `|n_z| > |n_x + n_y|` (the **sum**, as written), goes to
/// `unusedFacesBottom` when its centre is within 5 % of the root box's
/// height of the bottom, `unusedFacesTop` likewise at the top; the rest
/// stay in `defaultFaces`. Upstream's `FatalError` ("Cannot distribute the
/// face!!") is the `Err`. Upstream runs this at the end of `createMesh`;
/// [`cartesian_mesh_extractor`] leaves it to its caller (the 2D generator
/// calls it right after), so the 3D extractor stays infallible.
pub fn create_empty_patches(mesh: &VolumeMesh, root_box: &crate::mesh_octree::BoundBox) -> Result<(VolumeMesh, Vec<String>), String> {
    let t_z = 0.05 * (root_box.max.z - root_box.min.z);
    let n_int = mesh.n_internal_faces();
    let mut groups: [Vec<usize>; 3] = Default::default();
    for f in n_int..mesh.face_count() {
        let ring = &mesh.faces[f];
        let n = crate::surface_engine::ring_area_vector(&mesh.points, ring);
        if n.z.abs() > (n.x + n.y).abs() {
            let c = crate::correct_edges::foam_face_centre(&mesh.points, ring);
            if (c.z - root_box.min.z).abs() < t_z {
                groups[1].push(f);
            } else if (c.z - root_box.max.z).abs() < t_z {
                groups[2].push(f);
            } else {
                return Err(format!("Cannot distribute the face!! (face {f}, centre z {})", c.z));
            }
        } else {
            groups[0].push(f);
        }
    }
    let mut out = mesh.clone();
    out.faces.truncate(n_int);
    out.owner.truncate(n_int);
    out.neighbour.truncate(n_int);
    out.patches.clear();
    let names = ["defaultFaces", "unusedFacesBottom", "unusedFacesTop"];
    for (g, name) in groups.iter().zip(names) {
        let start = out.faces.len();
        for &f in g {
            out.faces.push(mesh.faces[f].clone());
            out.owner.push(mesh.owner[f]);
            out.neighbour.push(None);
        }
        out.patches.push(BoundaryPatch { name: name.to_string(), start_face: start, n_faces: g.len() });
    }
    Ok((out, vec!["patch".to_string(), "empty".to_string(), "empty".to_string()]))
}

/// `cartesianMeshExtractor::createMesh`: number the `MESHCELL` leaves as
/// cells, copy the octree faces between cells (internal) and between a cell
/// and anything else (boundary), optionally decompose the split hexes, and
/// drop unused points (`removeUnusedVertices`). Internal faces come first,
/// each oriented out of its lower-numbered cell.
pub fn cartesian_mesh_extractor(o: &MeshOctree, opts: &CartesianMeshOptions) -> CartesianMesh {
    extract_from_addressing(o, opts, MeshOctreeAddressing::new(o, opts.use_data_boxes))
}

/// `cartesianMeshExtractor(octree, meshDict, mesh)`: the extractor with
/// the addressing built from the dictionary
/// ([`MeshOctreeAddressing::from_dict`]: `nonManifoldMeshing`,
/// `keep`/`removeCellsIntersectingPatches`, `checkForGluedMesh`). With a
/// dict that sets none of those, identical to [`cartesian_mesh_extractor`].
pub fn cartesian_mesh_extractor_from_dict(o: &MeshOctree, opts: &CartesianMeshOptions, dict: &MeshDict) -> CartesianMesh {
    extract_from_addressing(o, opts, MeshOctreeAddressing::from_dict(o, dict, opts.use_data_boxes))
}

/// `polyMeshGenModifier::reorderBoundaryFaces` (serial): the boundary faces
/// among the first `nInternalFaces` faces are swapped, in order, with the
/// internal faces among the last ones. Faces and their owner / neighbour move
/// together; nothing else is reordered (gh:#1385: the template's face order
/// is upstream's, not a stable internal-then-boundary sort).
pub fn reorder_boundary_faces(faces: &mut [Vec<usize>], owner: &mut [usize], neighbour: &mut [Option<usize>]) {
    let n_int = neighbour.iter().filter(|x| x.is_some()).count();
    let to_boundary: Vec<usize> = (0..n_int).filter(|&f| neighbour[f].is_none()).collect();
    let to_internal: Vec<usize> = (n_int..faces.len()).filter(|&f| neighbour[f].is_some()).collect();
    assert_eq!(to_boundary.len(), to_internal.len());
    for (&a, &b) in to_boundary.iter().zip(&to_internal) {
        faces.swap(a, b);
        owner.swap(a, b);
        neighbour.swap(a, b);
    }
}

fn extract_from_addressing(o: &MeshOctree, opts: &CartesianMeshOptions, a: MeshOctreeAddressing) -> CartesianMesh {
    let n = o.number_of_leaves();
    let mut leaf_cell = vec![None; n];
    let mut n_cells = 0;
    for (l, c) in leaf_cell.iter_mut().enumerate() {
        if a.box_type[l] & BoxType::MESHCELL != 0 {
            *c = Some(n_cells);
            n_cells += 1;
        }
    }
    // `createPolyMesh`: the faces in octree-face order, internal and boundary
    // faces mixed; `reorderBoundaryFaces` then swaps them into place.
    let (mut faces, mut owner, mut neighbour) = (Vec::new(), Vec::new(), Vec::new());
    for f in 0..a.octree_faces.len() {
        let own = leaf_cell[a.octree_face_owner[f]];
        let nei = a.octree_face_neighbour[f].and_then(|l| leaf_cell[l]);
        match (own, nei) {
            (Some(oc), Some(nc)) => {
                faces.push(a.octree_faces[f].clone());
                owner.push(oc);
                neighbour.push(Some(nc));
            }
            (Some(oc), None) => {
                faces.push(a.octree_faces[f].clone());
                owner.push(oc);
                neighbour.push(None);
            }
            (None, Some(nc)) => {
                let mut r = a.octree_faces[f].clone();
                r[1..].reverse();
                faces.push(r);
                owner.push(nc);
                neighbour.push(None);
            }
            (None, None) => {}
        }
    }
    reorder_boundary_faces(&mut faces, &mut owner, &mut neighbour);
    let mut points = a.octree_points.clone();
    let mut n_decomposed = 0;
    if opts.decompose_split_hexes {
        let fan: Vec<bool> = faces.iter().map(|f| f.len() > 4).collect();
        // A cell is a hex if, after the fan, it has 6 faces, all quads.
        let mut n_faces = vec![0usize; n_cells];
        let mut all_quads = vec![true; n_cells];
        for (f, ring) in faces.iter().enumerate() {
            for c in std::iter::once(owner[f]).chain(neighbour[f]) {
                n_faces[c] += if fan[f] { ring.len() } else { 1 };
                if fan[f] || ring.len() != 4 {
                    all_quads[c] = false;
                }
            }
        }
        let decompose: Vec<bool> = (0..n_cells).map(|c| !(n_faces[c] == 6 && all_quads[c])).collect();
        n_decomposed = decompose.iter().filter(|&&d| d).count();
        let (p2, f2, o2, ne2, nc2) = crate::correct_edges::decompose_faces_and_cells(points, faces, owner, neighbour, n_cells, &fan, &decompose);
        points = p2;
        faces = f2;
        owner = o2;
        neighbour = ne2;
        n_cells = nc2;
    }
    // removeUnusedVertices, keeping the order of the kept points.
    let mut used = vec![false; points.len()];
    for f in &faces {
        for &p in f {
            used[p] = true;
        }
    }
    let mut new_id = vec![usize::MAX; points.len()];
    let mut kept = Vec::new();
    for (p, &u) in used.iter().enumerate() {
        if u {
            new_id[p] = kept.len();
            kept.push(points[p]);
        }
    }
    for f in faces.iter_mut() {
        for p in f.iter_mut() {
            *p = new_id[*p];
        }
    }
    let n_bnd = neighbour.iter().filter(|x| x.is_none()).count();
    let mesh = VolumeMesh {
        points: kept,
        patches: vec![BoundaryPatch { name: "defaultFaces".into(), start_face: faces.len() - n_bnd, n_faces: n_bnd }],
        faces,
        owner,
        neighbour,
        n_cells,
    };
    CartesianMesh { mesh, leaf_cell_label: leaf_cell, addressing: a, n_decomposed_cells: n_decomposed }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh_octree_creator::tests::{create, sphere3};
    use crate::mesh_octree_queries::tests::pipe;
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;

    /// Per-cell volumes by the divergence theorem over each cell's faces.
    fn cell_volumes(m: &VolumeMesh) -> Vec<f64> {
        let mut v = vec![0.0; m.n_cells];
        for f in 0..m.face_count() {
            let x = m.face_centroid(f).dot(m.face_area_vector(f)) / 3.0;
            v[m.owner[f]] += x;
            if let Some(n) = m.neighbour[f] {
                v[n] -= x;
            }
        }
        v
    }

    /// The faces of a mesh as point-coordinate rings in a canonical form
    /// (rotated to the smallest point, direction normalised), rounded to
    /// 1e-12 of `scale`, so two meshes with different point numbering can
    /// be compared face by face.
    fn canonical_faces(m: &VolumeMesh, scale: f64) -> BTreeMap<Vec<[i64; 3]>, usize> {
        let q = |p: Vec3| [(p.x / scale * 1e12).round() as i64, (p.y / scale * 1e12).round() as i64, (p.z / scale * 1e12).round() as i64];
        let mut out = BTreeMap::new();
        for f in &m.faces {
            let ring: Vec<[i64; 3]> = f.iter().map(|&p| q(m.points[p])).collect();
            let k = ring.len();
            let s = (0..k).min_by_key(|&i| ring[i]).unwrap();
            let fwd: Vec<[i64; 3]> = (0..k).map(|i| ring[(s + i) % k]).collect();
            let bwd: Vec<[i64; 3]> = (0..k).map(|i| ring[(s + k - i) % k]).collect();
            *out.entry(fwd.min(bwd)).or_insert(0) += 1;
        }
        out
    }

    /// The `MESHCELL` leaves as `octree.rs` cells, with the root as level 0.
    fn as_octree_rs(o: &MeshOctree, a: &MeshOctreeAddressing) -> VolumeMesh {
        let mut leaves: BTreeSet<crate::octree::Cell> = BTreeSet::new();
        let mut max_level = 0;
        for l in 0..o.number_of_leaves() {
            if a.box_type[l] & BoxType::MESHCELL != 0 {
                let c = o.return_leaf(l).coords;
                leaves.insert((c.level, c.x, c.y, c.z));
                max_level = max_level.max(c.level);
            }
        }
        crate::octree::build_mesh(&leaves, o.root_box.min, o.root_box.span().x, max_level)
    }

    /// V&V (gh:#1266), differential: `cartesianMeshExtractor` against
    /// `octree::build_mesh` on the **same leaves**.
    ///
    /// # Methodology
    ///
    /// Octrees from `createOctreeBoxes` on the sphere R 3 m (`maxCellSize`
    /// 1.0, `boundaryCellSize` 0.25) and the 48-sided pipe (4 mm / 1 mm).
    /// The upstream extraction keeps the `MESHCELL` leaves; the same leaf
    /// set is handed to `octree::build_mesh` (root as level 0).
    ///
    /// **Predictions (written before the run):**
    /// - same cell count, `validate()` on both, total volume equal to 1e-12
    ///   relative (upstream's points are the leaf corners inflated by
    ///   1e-15 x the root span; `octree.rs`'s are exact lattice points);
    /// - the same faces, ring for ring, compared by coordinates rounded to
    ///   1e-12 of the root span: both split a coarse face where it meets
    ///   finer cells and insert the hanging edge midpoints into the coarse
    ///   rings. The one possible difference: `octree.rs` inserts every
    ///   lattice point any face uses on an edge, upstream only the midpoint
    ///   a finer **used** leaf around the edge's first node supplies; on a
    ///   2:1 octree these should coincide.
    /// - every corner labelled (`n_unlabelled_corners` 0).
    ///
    /// # Results (2026-11-05)
    ///
    /// | surface | cells | faces | points | volume (m^3) | rel. diff | faces in one only | cells with > 6 faces |
    /// |---|---|---|---|---|---|---|---|
    /// | sphere | 3 248 / 3 248 | 11 532 / 11 532 | 5 073 / 5 073 | 91.875 / 91.875 | 2.5e-15 | 0 / 0 | 168 |
    /// | pipe 48 | 6 584 / 6 584 | 23 740 / 23 740 | 10 607 / 10 607 | 1.5544e-5 / 1.5544e-5 | 1.5e-14 | 0 / 0 | 656 |
    ///
    /// (upstream / `octree.rs`.) Every prediction held: the two routes build
    /// the same mesh up to point numbering, and the volume difference is the
    /// 1e-15 corner inflation. The counts are pinned in [`EXTRACT_PINS`].
    /// The volumes are the cube volumes of the kept leaves (the mesh is not
    /// yet mapped to the surface: 91.875 against the sphere's 113.097).
    /// (cells, faces, points, cells with more than 6 faces), sphere then pipe.
    const EXTRACT_PINS: [(usize, usize, usize, usize); 2] = [(3248, 11532, 5073, 168), (6584, 23740, 10607, 656)];

    #[test]
    fn extractor_matches_octree_rs_build_mesh_on_the_same_leaves() {
        for (name, surf, dict) in [
            ("sphere", sphere3(), MeshDict { boundary_cell_size: Some(0.25), ..MeshDict::with_max_cell_size(1.0) }),
            ("pipe", pipe(0.01, 0.06, 48), MeshDict { boundary_cell_size: Some(0.001), ..MeshDict::with_max_cell_size(0.004) }),
        ] {
            let (o, _) = create(surf, dict);
            let cm = cartesian_mesh_extractor(&o, &CartesianMeshOptions::default());
            let m = &cm.mesh;
            let r = as_octree_rs(&o, &cm.addressing);
            assert_eq!(cm.addressing.n_unlabelled_corners, 0, "{name}");
            m.validate().unwrap_or_else(|e| panic!("{name} upstream: {e}"));
            r.validate().unwrap_or_else(|e| panic!("{name} octree.rs: {e}"));
            let (vu, vr) = (m.total_volume(), r.total_volume());
            let scale = o.root_box.span().x;
            let (cu, cr) = (canonical_faces(m, scale), canonical_faces(&r, scale));
            let only_u: usize = cu.iter().filter(|(k, _)| !cr.contains_key(*k)).map(|(_, v)| v).sum();
            let only_r: usize = cr.iter().filter(|(k, _)| !cu.contains_key(*k)).map(|(_, v)| v).sum();
            let n_poly = {
                let mut nf = vec![0usize; m.n_cells];
                for f in 0..m.face_count() {
                    nf[m.owner[f]] += 1;
                    if let Some(n) = m.neighbour[f] {
                        nf[n] += 1;
                    }
                }
                nf.iter().filter(|&&k| k > 6).count()
            };
            println!(
                "{name}: cells {} / {}, faces {} / {}, points {} / {}, volume {vu:.12} / {vr:.12} (rel {:.2e}), \
                 faces only upstream {only_u}, only octree.rs {only_r}, split hexes {n_poly}",
                m.n_cells,
                r.n_cells,
                m.face_count(),
                r.face_count(),
                m.point_count(),
                r.point_count(),
                ((vu - vr) / vr).abs()
            );
            assert_eq!(m.n_cells, r.n_cells, "{name} cells");
            assert!(((vu - vr) / vr).abs() < 1e-12, "{name} volume");
            assert_eq!((only_u, only_r), (0, 0), "{name} faces");
            assert_eq!(m.face_count(), r.face_count(), "{name} face count");
            assert_eq!(m.point_count(), r.point_count(), "{name} point count");
            assert!(n_poly > 0, "{name}: the octree is graded, so some cells are split hexes");
            let pin = if name == "sphere" { EXTRACT_PINS[0] } else { EXTRACT_PINS[1] };
            assert_eq!((m.n_cells, m.face_count(), m.point_count(), n_poly), pin, "{name}");
        }
    }

    /// V&V (gh:#1266): `decomposeSplitHexes`. Every cell that is not a
    /// hexahedron (6 quads) becomes tetrahedra and pyramids whose volumes
    /// sum to the cell's; hexes are untouched; the mesh stays closed and
    /// the total volume is unchanged to 1e-12 relative.
    ///
    /// Result (2026-11-05), pipe: 6 584 cells -> 27 064, **864** decomposed
    /// (pinned), points 10 607 -> 12 975. The 864 are more than the 656
    /// cells with more than 6 faces: 208 cells have 6 faces but carry
    /// hanging points on their edges (faces of 5–8 points), and upstream
    /// fans every face over 4 points before matching hexes. A first version
    /// of this test counted "hex" as 6 faces and failed on exactly those.
    #[test]
    fn decompose_split_hexes_conserves_cell_volumes() {
        let (o, _) = create(pipe(0.01, 0.06, 48), MeshDict { boundary_cell_size: Some(0.001), ..MeshDict::with_max_cell_size(0.004) });
        let plain = cartesian_mesh_extractor(&o, &CartesianMeshOptions::default());
        let split = cartesian_mesh_extractor(&o, &CartesianMeshOptions { decompose_split_hexes: true, ..Default::default() });
        split.mesh.validate().expect("decomposed mesh closed");
        let (v0, v1) = (plain.mesh.total_volume(), split.mesh.total_volume());
        assert!(((v0 - v1) / v0).abs() < 1e-12, "{v0} {v1}");
        // Cells of the decomposed mesh: hexes first (kept order), then the
        // pyramids cell by cell; every new cell has 4 or 5 faces.
        let mut nf = vec![0usize; split.mesh.n_cells];
        for f in 0..split.mesh.face_count() {
            nf[split.mesh.owner[f]] += 1;
            if let Some(n) = split.mesh.neighbour[f] {
                nf[n] += 1;
            }
        }
        let n_kept = plain.mesh.n_cells - split.n_decomposed_cells;
        assert!(nf[..n_kept].iter().all(|&k| k == 6), "kept cells are hexes");
        assert!(nf[n_kept..].iter().all(|&k| k == 4 || k == 5), "new cells are tets and pyramids");
        assert!(cell_volumes(&split.mesh).iter().all(|&v| v > 0.0), "every cell positive");
        // The pyramids of each decomposed cell sum to its volume: compare the
        // volume of the kept-hex part.
        let vp = cell_volumes(&plain.mesh);
        let vs = cell_volumes(&split.mesh);
        let hex_plain: f64 = (0..plain.mesh.n_cells)
            .filter(|&c| {
                let fs: Vec<&Vec<usize>> = plain.mesh.faces.iter().enumerate().filter(|(f, _)| plain.mesh.owner[*f] == c || plain.mesh.neighbour[*f] == Some(c)).map(|(_, r)| r).collect();
                fs.len() == 6 && fs.iter().all(|r| r.len() == 4)
            })
            .map(|c| vp[c])
            .sum();
        let hex_split: f64 = vs[..n_kept].iter().sum();
        assert!(((hex_plain - hex_split) / hex_plain).abs() < 1e-12);
        println!(
            "pipe: {} cells -> {} ({} decomposed), points {} -> {}",
            plain.mesh.n_cells,
            split.mesh.n_cells,
            split.n_decomposed_cells,
            plain.mesh.point_count(),
            split.mesh.point_count()
        );
        assert_eq!((split.n_decomposed_cells, split.mesh.n_cells), (864, 27064));
    }

    /// V&V (gh:#1266): `keepCellsIntersectingBoundary` (`useDATABoxes`)
    /// keeps the leaves holding triangles as cells too: more cells, still
    /// closed, and every kept leaf's cell volume equals its cube volume.
    /// Node types: every node of an `INSIDE` cell without triangles is an
    /// `INNERNODE` or `OUTERNODE` (no untyped node of a `MESHCELL`), and
    /// `cubesAroundEdge` returns the leaf itself among the four.
    #[test]
    fn data_boxes_and_node_types() {
        let (o, _) = create(sphere3(), MeshDict { boundary_cell_size: Some(0.25), ..MeshDict::with_max_cell_size(1.0) });
        let plain = cartesian_mesh_extractor(&o, &CartesianMeshOptions::default());
        let with = cartesian_mesh_extractor(&o, &CartesianMeshOptions { use_data_boxes: true, ..Default::default() });
        with.mesh.validate().expect("closed");
        assert!(with.mesh.n_cells > plain.mesh.n_cells);
        let v = cell_volumes(&with.mesh);
        for l in 0..o.number_of_leaves() {
            if let Some(c) = with.leaf_cell_label[l] {
                let s = o.return_leaf(l).coords.size(&o.root_box);
                assert!((v[c] - s * s * s).abs() < 1e-12 * s * s * s, "cell {c} volume");
            }
        }
        let a = &plain.addressing;
        for l in 0..o.number_of_leaves() {
            if a.box_type[l] & BoxType::MESHCELL != 0 {
                for &n in &a.node_labels[l].unwrap() {
                    assert!(a.node_type[n] & (BoxType::INNERNODE | BoxType::OUTERNODE) != 0);
                }
                let around = a.cubes_around_edge(l, 0).unwrap();
                assert!(around.contains(&Some(l)));
            }
        }
        let opts = CartesianMeshOptions::from_dict(&MeshDict { keep_cells_intersecting_boundary: true, ..MeshDict::with_max_cell_size(1.0) });
        assert!(opts.use_data_boxes);
    }

    /// Connected components of the cell graph (over internal faces).
    fn cell_components(m: &VolumeMesh) -> usize {
        let mut parent: Vec<usize> = (0..m.n_cells).collect();
        fn find(p: &mut [usize], x: usize) -> usize {
            let mut r = x;
            while p[r] != r {
                r = p[r];
            }
            let mut y = x;
            while p[y] != r {
                let n = p[y];
                p[y] = r;
                y = n;
            }
            r
        }
        for f in 0..m.face_count() {
            if let Some(n) = m.neighbour[f] {
                let (a, b) = (find(&mut parent, m.owner[f]), find(&mut parent, n));
                if a != b {
                    parent[a] = b;
                }
            }
        }
        (0..m.n_cells).filter(|&c| find(&mut parent, c) == c).count()
    }

    /// Unordered point-coordinate pairs of a mesh's face edges, rounded.
    fn edge_keys(points: &[Vec3], faces: &[Vec<usize>], scale: f64) -> BTreeSet<([i64; 3], [i64; 3])> {
        let q = |p: Vec3| [(p.x / scale * 1e12).round() as i64, (p.y / scale * 1e12).round() as i64, (p.z / scale * 1e12).round() as i64];
        let mut out = BTreeSet::new();
        for f in faces {
            for i in 0..f.len() {
                let (a, b) = (q(points[f[i]]), q(points[f[(i + 1) % f.len()]]));
                out.insert((a.min(b), a.max(b)));
            }
        }
        out
    }

    /// V&V (gh:#1266 remainder), differential: the octree edges and their
    /// maps against the edges of the extracted mesh.
    ///
    /// **Predictions (written before the first run):** every octree face is
    /// a face of the extracted mesh (each has a `MESHCELL` side), so on the
    /// sphere and the pipe the octree edges are exactly the mesh's face
    /// edges (same count, same coordinate pairs); every edge has at least
    /// one face, each face's edge `i` joins its points `i`, `i + 1`; each
    /// internal mesh face appears once in `leafLeaves` per side; and
    /// `nodeFaces` lists every face at every node. `from_dict` with the
    /// default dict is `new` field for field.
    ///
    /// **Result (2026-11-05): as predicted.** Sphere 13 356 octree edges,
    /// pipe 27 762; each equal, pair for pair, to the extracted mesh's
    /// edges; every map check held.
    #[test]
    fn octree_edges_match_the_extracted_mesh_edges() {
        for (name, surf, dict) in [
            ("sphere", sphere3(), MeshDict { boundary_cell_size: Some(0.25), ..MeshDict::with_max_cell_size(1.0) }),
            ("pipe", pipe(0.01, 0.06, 48), MeshDict { boundary_cell_size: Some(0.001), ..MeshDict::with_max_cell_size(0.004) }),
        ] {
            let (o, _) = create(surf, dict);
            let a = MeshOctreeAddressing::new(&o, false);
            let b = MeshOctreeAddressing::from_dict(&o, &MeshDict::default(), false);
            assert_eq!((&a.box_type, &a.node_labels, &a.octree_faces, &a.octree_face_owner), (&b.box_type, &b.node_labels, &b.octree_faces, &b.octree_face_owner));
            let ea = a.edge_addressing();
            let cm = cartesian_mesh_extractor(&o, &CartesianMeshOptions::default());
            let scale = o.root_box.span().x;
            let oct = edge_keys(&a.octree_points, &a.octree_faces, scale);
            let msh = edge_keys(&cm.mesh.points, &cm.mesh.faces, scale);
            println!("{name}: octree edges {}, distinct by coordinates {}, mesh edges {}", ea.edges.len(), oct.len(), msh.len());
            assert_eq!(ea.edges.len(), oct.len());
            assert_eq!(oct, msh);
            for (f, fe) in ea.face_edges.iter().enumerate() {
                let ring = &a.octree_faces[f];
                for (i, &e) in fe.iter().enumerate() {
                    let (p, q) = (ring[i], ring[(i + 1) % ring.len()]);
                    let ed = ea.edges[e];
                    assert!((ed == [p, q]) || (ed == [q, p]));
                    assert!(ea.edge_faces[e].contains(&f));
                }
            }
            assert!(ea.edge_faces.iter().all(|ef| !ef.is_empty()));
            assert!(ea.edge_leaves.iter().all(|el| !el.is_empty() && el.len() <= 4));
            for (l, le) in ea.leaf_edges.iter().enumerate() {
                assert!(le.iter().all(|&e| ea.edge_leaves[e].contains(&l)));
            }
            let ll = a.leaf_leaves();
            let n_pairs: usize = ll.iter().map(|v| v.len()).sum();
            assert_eq!(n_pairs, 2 * a.octree_face_neighbour.iter().filter(|n| n.is_some()).count());
            let nf = a.node_faces();
            let total: usize = nf.iter().map(|v| v.len()).sum();
            assert_eq!(total, a.octree_faces.iter().map(|f| f.len()).sum::<usize>());
        }
    }

    /// V&V (gh:#1266 remainder), differential: `isIntersectedFace`,
    /// `isIntersectedEdge` and `edgeIntersections` against a brute force
    /// over **every** surface triangle.
    ///
    /// **Predictions (written before the first run):** sphere R 3 with
    /// DATA boxes kept. A triangle that geometrically crosses an internal
    /// octree face (`doFaceAndTriangleIntersect`, brute force over all
    /// triangles) lies in both the face's leaves, so `isIntersectedFace`
    /// reports every such face (0 missed); likewise every octree edge a
    /// triangle crosses (`triLineIntersection`) whose leaves all hold
    /// triangles is reported by `isIntersectedEdge`, with at least one
    /// `edgeIntersections` point. The reverse need not hold (two
    /// same-level leaves sharing a triangle count as crossed without a
    /// geometric test); counts recorded.
    ///
    /// **Result (2026-11-05): as predicted.** Faces: 5 268 crossed
    /// geometrically, 5 292 reported, 0 missed; edges: 2 634 crossed, 2 652
    /// reported, 0 missed, every crossed edge with an intersection point.
    /// 24 faces and 18 edges are reported but not crossed: upstream's
    /// same-level shortcut answers without the geometric test (each case
    /// not examined).
    #[test]
    fn intersected_faces_and_edges_against_brute_force() {
        let (o, _) = create(sphere3(), MeshDict { boundary_cell_size: Some(0.25), ..MeshDict::with_max_cell_size(1.0) });
        let a = MeshOctreeAddressing::new(&o, true);
        let ea = a.edge_addressing();
        let s = &o.surface;
        let (mut n_geo_f, mut n_rep_f, mut missed_f) = (0, 0, 0);
        for f in 0..a.octree_faces.len() {
            let Some(nei) = a.octree_face_neighbour[f] else {
                continue;
            };
            let own = a.octree_face_owner[f];
            let rep = a.is_intersected_face(&o, f);
            if rep {
                n_rep_f += 1;
                assert!(o.has_contained_triangles(own) && o.has_contained_triangles(nei));
            }
            if !(o.has_contained_triangles(own) && o.has_contained_triangles(nei)) {
                continue;
            }
            if (0..s.size()).any(|t| do_face_and_triangle_intersect(s, t, &a.octree_faces[f], &a.octree_points)) {
                n_geo_f += 1;
                if !rep {
                    missed_f += 1;
                }
            }
        }
        let (mut n_geo_e, mut n_rep_e, mut missed_e, mut no_point) = (0, 0, 0, 0);
        for e in 0..ea.edges.len() {
            let rep = a.is_intersected_edge(&o, &ea, e);
            if rep {
                n_rep_e += 1;
            }
            if ea.edge_leaves[e].iter().any(|&l| !o.has_contained_triangles(l)) {
                assert!(!rep);
                continue;
            }
            let [p, q] = ea.edges[e];
            let crossed = (0..s.size()).any(|t| {
                let tr = s.triangles[t];
                help::tri_line_intersection(s.points[tr[0]], s.points[tr[1]], s.points[tr[2]], a.octree_points[p], a.octree_points[q]).is_some()
            });
            if crossed {
                n_geo_e += 1;
                if !rep {
                    missed_e += 1;
                }
                if a.edge_intersections(&o, &ea, e).is_empty() {
                    no_point += 1;
                }
            }
        }
        println!(
            "sphere faces: reported {n_rep_f}, geometric {n_geo_f}, missed {missed_f}; edges: reported {n_rep_e}, geometric {n_geo_e}, missed {missed_e}, no intersection point {no_point}"
        );
        assert_eq!((missed_f, missed_e, no_point), (0, 0, 0));
        assert!(n_geo_f > 0 && n_geo_e > 0);
    }

    /// Two unit boxes `0.05` apart (each 12 triangles, one patch each).
    fn two_boxes() -> Arc<TriSurf> {
        let (mut p, mut t) = crate::shapes::box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let (p2, t2) = crate::shapes::box_surface(Vec3::new(1.05, 0.0, 0.0), Vec3::new(2.05, 1.0, 1.0));
        let off = p.len();
        p.extend(p2);
        t.extend(t2.iter().map(|tr| [tr[0] + off, tr[1] + off, tr[2] + off]));
        let regions = (0..t.len()).map(|i| i / 12).collect();
        Arc::new(TriSurf::new(p, t, Some(regions)))
    }

    /// V&V (gh:#1266 remainder): `checkForGluedMesh` (`checkGluedRegions`).
    ///
    /// # Methodology
    ///
    /// Two unit cubes 0.05 apart, `maxCellSize` 0.2 (4 x the gap),
    /// `keepCellsIntersectingBoundary` on. The root is sized to side 3.2
    /// (global level 4, cells 0.2) centred on `x = 1.025`, so a level
    /// plane runs through the middle of the gap and the leaves on each
    /// side hold one cube's face each.
    ///
    /// **Predictions (written before the first run):** without the check
    /// the DATA leaves either side of the gap are face neighbours and the
    /// mesh is **one** connected component (the cubes are glued); with it,
    /// a DATA leaf facing the gap has no inner corner at the gap (its gap
    /// nodes touch only DATA leaves), so its neighbour across the gap is
    /// not permissible and it becomes `BOUNDARY`: **two** components. The
    /// removed leaves at least cover the gap faces (25 per side, a 5 x 5
    /// patch of 0.2 cells over the unit face); total recorded.
    ///
    /// **Result (2026-11-05): as predicted.** 432 cells in one component
    /// without the check; with it 72 leaves become `BOUNDARY` and 360 cells
    /// remain in **two** components; the mesh is closed.
    #[test]
    fn glued_regions_are_separated() {
        let base = MeshDict { keep_cells_intersecting_boundary: true, ..MeshDict::with_max_cell_size(0.2) };
        let (o, _) = create(two_boxes(), base.clone());
        assert!((o.root_box.span().x - 3.2).abs() < 1e-12);
        let opts = CartesianMeshOptions::from_dict(&base);
        let glued = cartesian_mesh_extractor_from_dict(&o, &opts, &base);
        let checked_dict = MeshDict { check_for_glued_mesh: true, ..base.clone() };
        let mut a = MeshOctreeAddressing::from_dict(&o, &MeshDict { check_for_glued_mesh: false, ..base.clone() }, true);
        let removed = a.check_glued_regions(&o);
        let checked = cartesian_mesh_extractor_from_dict(&o, &opts, &checked_dict);
        checked.mesh.validate().expect("closed after the glued check");
        let (c0, c1) = (cell_components(&glued.mesh), cell_components(&checked.mesh));
        println!(
            "two boxes: cells {} -> {}, components {c0} -> {c1}, leaves made BOUNDARY {}",
            glued.mesh.n_cells,
            checked.mesh.n_cells,
            removed.len()
        );
        assert_eq!(glued.mesh.n_cells - checked.mesh.n_cells, removed.len());
        assert_eq!((c0, c1), (1, 2));
        assert!(removed.len() >= 50);
    }

    /// V&V (gh:#1266 remainder): `checkAndFixIrregularConnections`.
    ///
    /// **Predictions (written before the first run):** the node test never
    /// fires (module docs: upstream's `watertightSurface` is false at every
    /// node with a boundary face), so every pass reports 0 node
    /// irregularities; the passes end with (0, 0); the fixed mesh is
    /// closed and has no more cells than before. Edge counts on the sphere
    /// and the pipe (no DATA boxes) recorded, not predicted.
    ///
    /// **Result (2026-11-05):** one pass, (0, 0), on both: no octree edge
    /// carries more than two cell–boundary faces, cells unchanged (3 248
    /// and 6 584). The node prediction holds trivially here; it is shown by
    /// reading the code (module docs), not by a case that would have fired.
    #[test]
    fn irregular_connections_check() {
        for (name, surf, dict) in [
            ("sphere", sphere3(), MeshDict { boundary_cell_size: Some(0.25), ..MeshDict::with_max_cell_size(1.0) }),
            ("pipe", pipe(0.01, 0.06, 48), MeshDict { boundary_cell_size: Some(0.001), ..MeshDict::with_max_cell_size(0.004) }),
        ] {
            let (o, _) = create(surf, dict);
            let mut a = MeshOctreeAddressing::new(&o, false);
            let n0 = a.box_type.iter().filter(|&&b| b & BoxType::MESHCELL != 0).count();
            let passes = a.check_and_fix_irregular_connections(&o);
            let n1 = a.box_type.iter().filter(|&&b| b & BoxType::MESHCELL != 0).count();
            let m = extract_from_addressing(&o, &CartesianMeshOptions::default(), a);
            m.mesh.validate().unwrap_or_else(|e| panic!("{name}: {e}"));
            println!("{name}: irregular (edges, nodes) per pass {passes:?}; cells {n0} -> {n1}");
            assert!(passes.iter().all(|&(_, nodes)| nodes == 0));
            assert_eq!(passes.last(), Some(&(0, 0)));
            assert!(n1 <= n0);
            assert_eq!(m.mesh.n_cells, n1);
        }
    }

    /// V&V (gh:#1266 remainder): an irregular connection that fires.
    ///
    /// **Prediction (written before the first run):** on the sphere's
    /// octree, keep only two same-level `INSIDE` leaves that touch along
    /// one edge (diagonal in x-y) as cells, their face neighbours
    /// `BOUNDARY`. Their shared octree edge has four cell–`BOUNDARY` faces
    /// (two per cell), so pass 1 finds exactly 1 irregular edge and 0
    /// nodes, both cells become `BOUNDARY`, pass 2 finds (0, 0): passes
    /// `[(1, 0), (0, 0)]`, no cells left.
    ///
    /// **Result (2026-11-05): as predicted.** Leaves 362 and 419: passes
    /// `[(1, 0), (0, 0)]`, no cell left.
    #[test]
    fn irregular_edge_between_two_diagonal_cells_is_removed() {
        let (o, _) = create(sphere3(), MeshDict { boundary_cell_size: Some(0.25), ..MeshDict::with_max_cell_size(1.0) });
        let inside = |l: usize| !o.has_contained_triangles(l) && o.return_leaf(l).cube_type & CubeType::INSIDE != 0;
        let (a, b) = (0..o.number_of_leaves())
            .filter(|&l| inside(l))
            .find_map(|l| {
                let cc = o.return_leaf(l).coords;
                let d = crate::mesh_octree::CubeCoordinates::new(cc.x + 1, cc.y + 1, cc.z, cc.level);
                let m = o.find_leaf_label_for_position(&d)?;
                (inside(m) && o.return_leaf(m).coords.level == cc.level).then_some((l, m))
            })
            .expect("a diagonal pair");
        let mut bt = vec![BoxType::NONE; o.number_of_leaves()];
        bt[a] = BoxType::MESHCELL;
        bt[b] = BoxType::MESHCELL;
        for l in [a, b] {
            for nei in o.find_neighbours_for_leaf(&o.return_leaf(l).coords).into_iter().flatten() {
                if bt[nei] & BoxType::MESHCELL == 0 {
                    bt[nei] = BoxType::BOUNDARY;
                }
            }
        }
        let mut addr = MeshOctreeAddressing::from_box_type(&o, false, bt);
        let passes = addr.check_and_fix_irregular_connections(&o);
        println!("diagonal pair {a}, {b}: passes {passes:?}");
        assert_eq!(passes, vec![(1, 0), (0, 0)]);
        assert!(addr.box_type.iter().all(|&t| t & BoxType::MESHCELL == 0));
    }

    /// V&V (gh:#1266 remainder): `nonManifoldMeshing`.
    ///
    /// **Predictions (written before the first run):** on the sphere (no
    /// `UNKNOWN` leaves, gh:#1264) `nonManifoldMeshing` gives the same
    /// mesh as `keepCellsIntersectingBoundary`. With one `INSIDE` leaf
    /// without triangles retyped `UNKNOWN`, the plain DATA-box mesh loses
    /// exactly that cell and the non-manifold one keeps it.
    ///
    /// **Result (2026-11-05): as predicted** (5 896 = 5 896; then 5 896
    /// and 5 895).
    #[test]
    fn non_manifold_meshing_keeps_unknown_leaves() {
        let (mut o, _) = create(sphere3(), MeshDict { boundary_cell_size: Some(0.25), ..MeshDict::with_max_cell_size(1.0) });
        let opts = CartesianMeshOptions::default();
        let nm = MeshDict { non_manifold_meshing: true, ..MeshDict::default() };
        let keep = MeshDict { keep_cells_intersecting_boundary: true, ..MeshDict::default() };
        let a = cartesian_mesh_extractor_from_dict(&o, &opts, &nm).mesh;
        let b = cartesian_mesh_extractor_from_dict(&o, &opts, &keep).mesh;
        assert_eq!((a.n_cells, &a.faces, &a.points), (b.n_cells, &b.faces, &b.points));
        let l = (0..o.number_of_leaves()).find(|&l| !o.has_contained_triangles(l) && o.return_leaf(l).cube_type & CubeType::INSIDE != 0).unwrap();
        let ci = o.leaves[l];
        o.slot.cubes[ci].cube_type = CubeType::UNKNOWN;
        let a2 = cartesian_mesh_extractor_from_dict(&o, &opts, &nm).mesh;
        let b2 = cartesian_mesh_extractor_from_dict(&o, &opts, &keep).mesh;
        println!("non-manifold: cells {} (keep {}); with one UNKNOWN leaf {} (keep {})", a.n_cells, b.n_cells, a2.n_cells, b2.n_cells);
        assert_eq!((a2.n_cells, b2.n_cells), (a.n_cells, a.n_cells - 1));
        a2.validate().expect("closed");
    }

    /// V&V (gh:#1266 remainder): `keepCellsIntersectingPatches` and
    /// `removeCellsIntersectingPatches`, by patch name and by facet-subset
    /// name.
    ///
    /// **Predictions (written before the first run):** pipe, inlet patch
    /// (region 1). `removeCellsIntersectingPatches` [inlet] with DATA boxes
    /// removes exactly the leaves holding an inlet triangle from the DATA
    /// mesh; `keepCellsIntersectingPatches` [inlet] without DATA boxes adds
    /// exactly those leaves to the default mesh (none of them is a cell by
    /// default: they hold triangles). A facet subset holding the inlet's
    /// facets gives the same meshes. Both meshes are closed. Keep is
    /// ignored with DATA boxes, remove without (upstream's `else if`).
    ///
    /// **Result (2026-11-05): as predicted.** 704 leaves hold an inlet
    /// triangle; cells 6 584 -> 7 288 with keep, 12 864 -> 12 160 with
    /// remove; the subset name gives the same faces; both meshes closed.
    #[test]
    fn keep_and_remove_cells_intersecting_patches() {
        let mut s = (*pipe(0.01, 0.06, 48)).clone();
        let inlet = s.patches[1].name.clone();
        let id = s.facet_subsets.add("inlet_set");
        for t in 0..s.size() {
            if s.regions[t] == 1 {
                s.facet_subsets.add_element(id, t);
            }
        }
        let (o, _) = create(Arc::new(s), MeshDict { boundary_cell_size: Some(0.001), ..MeshDict::with_max_cell_size(0.004) });
        let n_inlet = (0..o.number_of_leaves()).filter(|&l| o.contained_triangles(l).iter().any(|&t| o.surface.regions[t] == 1)).count();
        let opts = CartesianMeshOptions::default();
        let run = |d: MeshDict| cartesian_mesh_extractor_from_dict(&o, &opts, &d).mesh;
        let plain = run(MeshDict::default());
        let data = run(MeshDict { keep_cells_intersecting_boundary: true, ..MeshDict::default() });
        let removed = run(MeshDict { keep_cells_intersecting_boundary: true, remove_cells_intersecting_patches: vec![inlet.clone()], ..MeshDict::default() });
        let removed_s = run(MeshDict { keep_cells_intersecting_boundary: true, remove_cells_intersecting_patches: vec!["inlet_set".into()], ..MeshDict::default() });
        let kept = run(MeshDict { keep_cells_intersecting_patches: vec![inlet.clone()], ..MeshDict::default() });
        let kept_s = run(MeshDict { keep_cells_intersecting_patches: vec!["inlet_set".into()], ..MeshDict::default() });
        let ignored = run(MeshDict { keep_cells_intersecting_boundary: true, keep_cells_intersecting_patches: vec![inlet], ..MeshDict::default() });
        println!(
            "pipe: inlet leaves {n_inlet}; cells plain {}, keep [inlet] {}, DATA {}, DATA remove [inlet] {}",
            plain.n_cells, kept.n_cells, data.n_cells, removed.n_cells
        );
        assert_eq!(removed.n_cells, data.n_cells - n_inlet);
        assert_eq!(kept.n_cells, plain.n_cells + n_inlet);
        assert_eq!((&removed.faces, &kept.faces), (&removed_s.faces, &kept_s.faces));
        assert_eq!(ignored.faces, data.faces);
        removed.validate().expect("remove closed");
        kept.validate().expect("keep closed");
    }
}
