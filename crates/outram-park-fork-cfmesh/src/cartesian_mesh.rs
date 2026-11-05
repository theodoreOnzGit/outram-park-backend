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
//! # Not ported
//!
//! `nonManifoldMeshing`, `removeCellsIntersectingPatches`,
//! `keepCellsIntersectingPatches`, `checkGluedRegions`
//! (`checkForGluedMesh`), `checkAndFixIrregularConnections`, the octree
//! edges and their addressing (`createOctreeEdges` and the five
//! edge-related maps), `isIntersectedFace` / `isIntersectedEdge`,
//! `leafLeaves`, `nodeFaces` ~~, the quadtree's empty patches~~ and
//! everything parallel. All boundary faces of an octree go to one patch,
//! `defaultFaces` (upstream's `polyMeshGen` leaves them unnamed until the
//! generator assigns patches). **Quadtree (gh:#1275):** the faces whose
//! normal is z-dominated go to the empty patches `unusedFacesBottom` /
//! `unusedFacesTop` ([`create_empty_patches`]).

use crate::math::Vec3;
use crate::mesh_octree::{CubeType, MeshOctree, EDGE_NODES, FACE_EDGES, FACE_NODES};
use crate::mesh_octree_creator::MeshDict;
use crate::volume_mesh::{BoundaryPatch, VolumeMesh};

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
        let n = o.number_of_leaves();
        let box_type = find_used_boxes(o, use_data_boxes);
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
}

/// `findUsedBoxes` (module docs).
fn find_used_boxes(o: &MeshOctree, use_data_boxes: bool) -> Vec<u8> {
    let n = o.number_of_leaves();
    let mut bt = vec![BoxType::NONE; n];
    for (l, b) in bt.iter_mut().enumerate() {
        let leaf = o.return_leaf(l);
        if !o.has_contained_triangles(l) && !o.has_contained_edges(l) && leaf.cube_type & CubeType::INSIDE != 0 {
            *b |= BoxType::MESHCELL;
        }
    }
    if use_data_boxes {
        for (l, b) in bt.iter_mut().enumerate() {
            if o.has_contained_triangles(l) || o.has_contained_edges(l) {
                *b |= BoxType::MESHCELL;
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
    let a = MeshOctreeAddressing::new(o, opts.use_data_boxes);
    let n = o.number_of_leaves();
    let mut leaf_cell = vec![None; n];
    let mut n_cells = 0;
    for (l, c) in leaf_cell.iter_mut().enumerate() {
        if a.box_type[l] & BoxType::MESHCELL != 0 {
            *c = Some(n_cells);
            n_cells += 1;
        }
    }
    let (mut int_f, mut int_o, mut int_n) = (Vec::new(), Vec::new(), Vec::new());
    let (mut bnd_f, mut bnd_o) = (Vec::new(), Vec::new());
    for f in 0..a.octree_faces.len() {
        let own = leaf_cell[a.octree_face_owner[f]];
        let nei = a.octree_face_neighbour[f].and_then(|l| leaf_cell[l]);
        match (own, nei) {
            (Some(oc), Some(nc)) => {
                int_f.push(a.octree_faces[f].clone());
                int_o.push(oc);
                int_n.push(Some(nc));
            }
            (Some(oc), None) => {
                bnd_f.push(a.octree_faces[f].clone());
                bnd_o.push(oc);
            }
            (None, Some(nc)) => {
                let mut r = a.octree_faces[f].clone();
                r[1..].reverse();
                bnd_f.push(r);
                bnd_o.push(nc);
            }
            (None, None) => {}
        }
    }
    let n_int = int_f.len();
    let mut faces = int_f;
    faces.extend(bnd_f);
    let mut owner = int_o;
    owner.extend(bnd_o);
    let mut neighbour = int_n;
    neighbour.extend(std::iter::repeat(None).take(faces.len() - n_int));
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
}
