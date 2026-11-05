// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/tetrahedra/tetCreatorOctree/
//     tetCreatorOctree.{C,H}, tetCreatorOctreeI.H,
//     tetCreatorOctreePointsAndAddressing.C,
//     tetCreatorOctreeFromFacesWithCentreNode.C,
//     tetCreatorOctreeTetsAroundEdges.C,
//     tetCreatorOctreeTetsAroundSplitEdges.C,
//     tetCreatorOctreeTetsFromSplitFaces.C
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

//! cfMesh's **`tetCreatorOctree`** (gh:#1272): tetrahedra built from the
//! octree's leaves, the tets that feed upstream's `tetMesh` and `pMesh`
//! workflows.
//!
//! The points are, in this order (upstream's numbering):
//!
//! 1. the octree nodes (`meshOctreeAddressing::nodeLabels`, with
//!    `useDATABoxes` on, so leaves holding triangles are cells too);
//! 2. the centre of every `MESHCELL` leaf (`selectElements`);
//! 3. "sub-nodes": at a node where leaves of different levels meet, the
//!    midpoint of each coarser leaf's corner and its centre
//!    (`createPointsAndAddressing`);
//! 4. face centres of leaves next to finer ones (`createFaceCentreLabels`,
//!    then `createTetsFromFacesWithCentreNode`);
//! 5. edge centres of edges whose ends sit at different levels
//!    (`createTetsAroundSplitEdges`).
//!
//! The tets, in the order upstream creates them: from faces with a centre
//! node, around split edges, around regular edges (two leaf centres and
//! the edge: on a uniform octree every tet is of this kind, the BCC
//! lattice), and from split faces.
//!
//! # Where this port differs, and why
//!
//! Upstream reads a few rows that can be empty or labels that can be `-1`
//! as if they were set (`VRWGraph::operator()` has no bound check in a
//! release build, and `tetPoints_[-1]` is out of range). The port treats an
//! empty row or a missing label as `-1` and counts every such read in
//! [`TetCreatorOctree::defects`], so a run that would have been undefined
//! behaviour upstream says so instead of producing a tet from garbage. Where
//! upstream stops with `FatalError` ("Cannot find cube position"), the port
//! counts the event and leaves that centre out.

use crate::cartesian_mesh::{BoxType, MeshOctreeAddressing};
use crate::math::Vec3;
use crate::mesh_octree::{MeshOctree, EDGE_FACES, EDGE_NODES, FACE_EDGES, FACE_NODES};

/// cfMesh `label`: `-1` is "none".
type Label = i64;

/// `tetCreatorOctree::faceCentreHelper_`.
const FACE_CENTRE_HELPER: [[usize; 4]; 3] = [[3, 5, 2, 4], [5, 1, 4, 0], [1, 3, 0, 2]];

/// Reads upstream makes that are undefined in a release build, or that stop
/// it (module docs). All zero on the sphere and the pipe (see the tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TetCreatorDefects {
    /// A sub-node or face-centre row that was never sized was read.
    pub empty_row_reads: usize,
    /// A node label of `-1` (an unlabelled corner) was used.
    pub unlabelled_node_reads: usize,
    /// `createTetsAroundEdges` would have appended a tet with a `-1` vertex
    /// (upstream appends it unchecked).
    pub invalid_edge_tets: usize,
    /// "Cannot find cube position" (`FatalError` upstream).
    pub missing_cube_positions: usize,
}

/// The tetrahedra of `tetCreatorOctree`.
#[derive(Debug, Clone, PartialEq)]
pub struct TetCreatorOctree {
    /// `tetPoints()`.
    pub tet_points: Vec<Vec3>,
    /// `tets()`: `partTet(a, b, c, d)`, positive when `d` is on the side
    /// `(b − a) × (c − a)` points to.
    pub tets: Vec<[usize; 4]>,
    /// Number of octree nodes (the first points).
    pub n_nodes: usize,
    /// Tets `checkAndAppendTet` dropped because a vertex was `-1`.
    pub n_dropped_tets: usize,
    /// Reads upstream does not guard (module docs).
    pub defects: TetCreatorDefects,
}

/// Working state of the creator (upstream's private members).
struct Creator<'a> {
    o: &'a MeshOctree,
    a: &'a MeshOctreeAddressing,
    tet_points: Vec<Vec3>,
    tets: Vec<[Label; 4]>,
    sorted_leaves: Vec<Vec<usize>>,
    sub_node_labels: Vec<Option<[Label; 8]>>,
    cube_label: Vec<Label>,
    face_centre_label: Vec<Option<[Label; 6]>>,
    n_dropped: usize,
    defects: TetCreatorDefects,
}

/// `tetCreatorOctree(octree, meshDict)`: the addressing with `useDATABoxes`
/// on (upstream passes `true`), then `createTets`.
pub fn tet_creator_octree(o: &MeshOctree) -> TetCreatorOctree {
    let a = MeshOctreeAddressing::new(o, true);
    tet_creator_octree_with(o, &a)
}

/// [`tet_creator_octree`] on an addressing already built (it must have been
/// built with `use_data_boxes` as the caller wants; upstream's is `true`).
pub fn tet_creator_octree_with(o: &MeshOctree, a: &MeshOctreeAddressing) -> TetCreatorOctree {
    let n_leaves = o.number_of_leaves();
    let mut c = Creator {
        o,
        a,
        tet_points: Vec::new(),
        tets: Vec::new(),
        sorted_leaves: Vec::new(),
        sub_node_labels: vec![None; n_leaves],
        cube_label: Vec::new(),
        face_centre_label: vec![None; n_leaves],
        n_dropped: 0,
        defects: TetCreatorDefects::default(),
    };
    // createTets
    c.create_points_and_addressing();
    c.create_tets_from_faces_with_centre_node();
    c.create_tets_around_split_edges();
    c.create_tets_around_edges();
    c.create_tets_from_split_faces();
    let tets = c.tets.iter().map(|t| [t[0] as usize, t[1] as usize, t[2] as usize, t[3] as usize]).collect();
    TetCreatorOctree { tet_points: c.tet_points, tets, n_nodes: a.number_of_nodes(), n_dropped_tets: c.n_dropped, defects: c.defects }
}

impl Creator<'_> {
    /// `nodeLabels(leaf, k)`, `-1` for an unlabelled corner or a leaf with
    /// no row.
    fn node_label(&mut self, leaf: usize, k: usize) -> Label {
        match self.a.node_labels[leaf] {
            Some(row) if row[k] != usize::MAX => row[k] as Label,
            Some(_) => {
                self.defects.unlabelled_node_reads += 1;
                -1
            }
            None => {
                self.defects.empty_row_reads += 1;
                -1
            }
        }
    }

    /// `nodeLeaves(node, k)`, `-1` for none.
    fn node_leaf(&self, node: usize, k: usize) -> Label {
        self.a.node_leaves[node][k].map_or(-1, |l| l as Label)
    }

    /// `subNodeLabels(leaf, k)`; an empty row reads `-1` (module docs).
    fn sub_node(&mut self, leaf: Label, k: usize) -> Label {
        if leaf < 0 {
            self.defects.empty_row_reads += 1;
            return -1;
        }
        match self.sub_node_labels[leaf as usize] {
            Some(r) => r[k],
            None => {
                self.defects.empty_row_reads += 1;
                -1
            }
        }
    }

    fn level(&self, leaf: usize) -> u8 {
        self.o.return_leaf(leaf).coords.level
    }

    /// `checkAndAppendTet`.
    fn check_and_append_tet(&mut self, t: [Label; 4]) {
        if t.iter().all(|&v| v >= 0) {
            self.tets.push(t);
        } else {
            self.n_dropped += 1;
        }
    }

    /// `selectElements`.
    fn select_elements(&mut self) {
        let n_nodes = self.a.number_of_nodes();
        self.tet_points = vec![Vec3::ZERO; n_nodes];
        for leaf in 0..self.o.number_of_leaves() {
            if let Some(row) = self.a.node_labels[leaf] {
                let lv = self.o.return_leaf(leaf).coords.vertices(&self.o.root_box);
                for (k, &node) in row.iter().enumerate() {
                    if node != usize::MAX {
                        self.tet_points[node] = lv[k];
                    } else {
                        self.defects.unlabelled_node_reads += 1;
                    }
                }
            }
        }
        self.cube_label = vec![-1; self.o.number_of_leaves()];
        for leaf in 0..self.o.number_of_leaves() {
            if self.a.box_type[leaf] & BoxType::MESHCELL != 0 {
                self.cube_label[leaf] = self.tet_points.len() as Label;
                self.tet_points.push(self.o.return_leaf(leaf).coords.centre(&self.o.root_box));
            }
        }
    }

    /// `createPointsAndAddressing`.
    fn create_points_and_addressing(&mut self) {
        self.select_elements();
        let n_leaves = self.o.number_of_leaves();
        let max_level = (0..n_leaves).map(|l| self.level(l)).max().unwrap_or(0);
        self.sorted_leaves = vec![Vec::new(); max_level as usize + 1];
        for l in 0..n_leaves {
            let lv = self.level(l) as usize;
            self.sorted_leaves[lv].push(l);
        }
        // subNodeLabels
        for point in 0..self.a.node_leaves.len() {
            let pl = self.a.node_leaves[point];
            let mut valid = [false; 8];
            let mut level_i = 0u8;
            for i in 0..8 {
                if let Some(li) = pl[i] {
                    valid[i] = true;
                    for j in i + 1..8 {
                        if pl[j] == Some(li) {
                            valid[i] = false;
                            valid[j] = false;
                        }
                    }
                    level_i = level_i.max(self.level(li));
                }
            }
            for pli in 0..8 {
                if !valid[pli] {
                    continue;
                }
                let leaf = pl[pli].expect("valid leaf");
                let cc = self.o.return_leaf(leaf).coords;
                if cc.level < level_i {
                    let row = self.sub_node_labels[leaf].get_or_insert([-1; 8]);
                    row[7 - pli] = self.tet_points.len() as Label;
                    let lv = cc.vertices(&self.o.root_box);
                    let centre = cc.centre(&self.o.root_box);
                    self.tet_points.push(lv[7 - pli].add(centre).scale(0.5));
                }
            }
        }
        self.create_face_centre_labels();
    }

    /// `createFaceCentreLabels`.
    fn create_face_centre_labels(&mut self) {
        let n_nodes = self.a.node_leaves.len();
        let mut node_level = vec![0u8; n_nodes];
        for leaf in 0..self.o.number_of_leaves() {
            let level = self.level(leaf);
            if let Some(row) = self.a.node_labels[leaf] {
                for &n in row.iter() {
                    if n != usize::MAX && node_level[n] < level {
                        node_level[n] = level;
                    }
                }
            }
        }
        for cube in 0..self.cube_label.len() {
            if self.cube_label[cube] == -1 {
                continue;
            }
            let level = self.level(cube);
            for i in 0..6 {
                if let Some(r) = self.face_centre_label[cube] {
                    if r[i] != -1 {
                        continue;
                    }
                }
                let mut high_level_node = None;
                for j in 0..4 {
                    let n = self.node_label(cube, FACE_NODES[i][j]);
                    if n < 0 {
                        continue;
                    }
                    if node_level[n as usize] > level {
                        high_level_node = Some(j);
                        break;
                    }
                }
                if high_level_node.is_none() {
                    continue;
                }
                let mut neis = Vec::new();
                let cc = self.o.return_leaf(cube).coords;
                self.o.find_neighbours_in_direction(&cc, i, &mut neis);
                if neis.len() != 1 {
                    continue;
                }
                let Some(c_nei) = neis[0] else {
                    continue;
                };
                let label = self.tet_points.len() as Label;
                self.face_centre_label[cube].get_or_insert([-1; 6])[i] = label;
                // the opposite face of the neighbour: 0<->1, 2<->3, 4<->5
                self.face_centre_label[c_nei].get_or_insert([-1; 6])[i ^ 1] = label;
                let mut p = Vec3::ZERO;
                for j in 0..4 {
                    let n = self.node_label(cube, FACE_NODES[i][j]);
                    if n >= 0 {
                        p = p.add(self.tet_points[n as usize]);
                    }
                }
                self.tet_points.push(p.scale(1.0 / 4.0));
            }
        }
    }

    /// `createTetsFromFacesWithCentreNode`.
    fn create_tets_from_faces_with_centre_node(&mut self) {
        for point in 0..self.a.node_leaves.len() {
            let mut pl = [0usize; 8];
            let mut create = true;
            for i in 0..8 {
                match self.a.node_leaves[point][i] {
                    Some(l) => pl[i] = l,
                    None => {
                        create = false;
                        break;
                    }
                }
            }
            if !create {
                continue;
            }
            for fi in 0..6 {
                let f_edges = FACE_EDGES[fi];
                for fe_i in 0..2 {
                    let fe_j = fe_i + 2;
                    let s_edge = EDGE_NODES[f_edges[fe_i]];
                    let e_edge = EDGE_NODES[f_edges[fe_j]];
                    let sp = s_edge[0];
                    let ep = e_edge[0];
                    if pl[sp] == pl[ep] || pl[sp] != pl[s_edge[1]] || pl[ep] != pl[e_edge[1]] {
                        continue;
                    }
                    self.face_centre_label[pl[sp]].get_or_insert([-1; 6]);
                    self.face_centre_label[pl[ep]].get_or_insert([-1; 6]);
                    let mut fs = EDGE_FACES[f_edges[fe_j]][0];
                    if fs == fi {
                        fs = EDGE_FACES[f_edges[fe_j]][1];
                    }
                    let mut fe = EDGE_FACES[f_edges[fe_i]][0];
                    if fe == fi {
                        fe = EDGE_FACES[f_edges[fe_i]][1];
                    }
                    if self.face_centre_label[pl[sp]].expect("sized")[fs] == -1 {
                        let label = self.tet_points.len() as Label;
                        self.face_centre_label[pl[sp]].as_mut().expect("sized")[fs] = label;
                        self.face_centre_label[pl[ep]].as_mut().expect("sized")[fe] = label;
                        let mut p = Vec3::ZERO;
                        for i in 0..4 {
                            let n = self.node_label(pl[ep], FACE_NODES[fe][i]);
                            if n >= 0 {
                                p = p.add(self.tet_points[n as usize]);
                            }
                        }
                        self.tet_points.push(p.scale(1.0 / 4.0));
                    }
                    let fc = self.face_centre_label[pl[sp]].expect("sized")[fs];
                    let (lsp, lep) = (pl[sp] as Label, pl[ep] as Label);
                    let t1 = [fc, self.sub_node(lep, 7 - e_edge[0]), self.sub_node(lep, 7 - e_edge[1]), self.cube_label[pl[ep]]];
                    self.check_and_append_tet(t1);
                    let t2 = [fc, self.sub_node(lsp, 7 - s_edge[1]), self.sub_node(lsp, 7 - s_edge[0]), self.cube_label[pl[sp]]];
                    self.check_and_append_tet(t2);
                    let sub_nodes = [
                        self.sub_node(lsp, 7 - s_edge[1]),
                        self.sub_node(lsp, 7 - s_edge[0]),
                        self.sub_node(lep, 7 - e_edge[0]),
                        self.sub_node(lep, 7 - e_edge[1]),
                    ];
                    // faceCentreLabel(pl[sEdge[0]], fs): sEdge[0] is sp
                    let fc0 = self.face_centre_label[pl[s_edge[0]]].expect("sized")[fs];
                    for n in 0..4 {
                        self.check_and_append_tet([point as Label, sub_nodes[n], sub_nodes[(n + 1) % 4], fc0]);
                    }
                }
            }
        }
    }

    /// `nodeLevel` of the tet routines: the finest used leaf at each node.
    fn node_level_from_leaves(&self) -> Vec<u8> {
        self.a.node_leaves.iter().map(|pl| pl.iter().flatten().map(|&l| self.level(l)).max().unwrap_or(0)).collect()
    }

    /// `faceCentreLabel(leaf, k)` where a missing row reads `-1`.
    fn face_centre(&self, leaf: usize, k: usize) -> Label {
        self.face_centre_label[leaf].map_or(-1, |r| r[k])
    }

    /// `createTetsAroundSplitEdges`.
    fn create_tets_around_split_edges(&mut self) {
        let node_level = self.node_level_from_leaves();
        let mut help = [[-1 as Label; 8]; 2];
        let mut face_centres = [-1 as Label; 4];
        for level_i in (0..self.sorted_leaves.len()).rev() {
            let level = level_i as u8;
            for li in 0..self.sorted_leaves[level_i].len() {
                let cur = self.sorted_leaves[level_i][li];
                if self.cube_label[cur] == -1 {
                    continue;
                }
                for ei in 0..12 {
                    let start = self.node_label(cur, EDGE_NODES[ei][0]);
                    let end = self.node_label(cur, EDGE_NODES[ei][1]);
                    if start < 0 || end < 0 {
                        continue;
                    }
                    let (s, e) = (start as usize, end as usize);
                    if node_level[s] == level && node_level[e] == level {
                        continue;
                    }
                    let mut create = true;
                    let fi = 2 * (ei / 4) + 1;
                    let f_nodes = FACE_NODES[fi];
                    let edge_cubes: [Label; 4] = std::array::from_fn(|i| self.node_leaf(s, f_nodes[i]));
                    for i in 0..4 {
                        let cl = edge_cubes[i];
                        if cl == -1 || cl < cur as Label || self.level(cl as usize) != level {
                            create = false;
                            break;
                        }
                        let cl = cl as usize;
                        for j in 0..8 {
                            let nl = self.node_label(cl, j);
                            if nl == start {
                                help[0][i] = match self.sub_node_labels[cl] {
                                    Some(r) => r[j],
                                    None => -1,
                                };
                                help[0][i + 4] = self.cube_label[cl];
                            }
                            if nl == end {
                                help[1][i + 4] = match self.sub_node_labels[cl] {
                                    Some(r) => r[j],
                                    None => -1,
                                };
                                help[1][i] = self.cube_label[cl];
                            }
                        }
                        face_centres[i] = self.face_centre(cl, FACE_CENTRE_HELPER[ei / 4][i]);
                    }
                    if !create {
                        continue;
                    }
                    let ec = self.tet_points.len() as Label;
                    if node_level[s] > level {
                        for k in 0..4 {
                            self.check_and_append_tet([start, help[0][k], help[0][(k + 1) % 4], ec]);
                            self.check_and_append_tet([ec, help[0][k], help[0][(k + 1) % 4], face_centres[k]]);
                            self.check_and_append_tet([face_centres[k], help[0][k], help[0][k + 4], ec]);
                            self.check_and_append_tet([face_centres[(k + 3) % 4], help[0][k + 4], help[0][k], ec]);
                        }
                    } else {
                        for k in 0..4 {
                            self.check_and_append_tet([face_centres[(k + 3) % 4], help[0][k + 4], start, ec]);
                            self.check_and_append_tet([face_centres[k], start, help[0][k + 4], ec]);
                        }
                    }
                    if node_level[e] > level {
                        for k in 0..4 {
                            self.check_and_append_tet([ec, help[1][k + 4], help[1][((k + 1) % 4) + 4], end]);
                            self.check_and_append_tet([face_centres[k], help[1][k + 4], help[1][((k + 1) % 4) + 4], ec]);
                            self.check_and_append_tet([ec, help[1][k], face_centres[k], help[1][k + 4]]);
                            self.check_and_append_tet([face_centres[(k + 3) % 4], help[1][k], ec, help[1][k + 4]]);
                        }
                    } else {
                        for k in 0..4 {
                            self.check_and_append_tet([face_centres[(k + 3) % 4], help[1][k], ec, end]);
                            self.check_and_append_tet([help[1][k], face_centres[k], ec, end]);
                        }
                    }
                    let mid = self.tet_points[s].add(self.tet_points[e]).scale(0.5);
                    self.tet_points.push(mid);
                }
            }
        }
    }

    /// `createTetsAroundEdges`.
    fn create_tets_around_edges(&mut self) {
        let node_level = self.node_level_from_leaves();
        let vl_pos = self.o.vrt_leaves_pos;
        for level_i in (0..self.sorted_leaves.len()).rev() {
            let level = level_i as u8;
            for li in 0..self.sorted_leaves[level_i].len() {
                let cur = self.sorted_leaves[level_i][li];
                if self.cube_label[cur] == -1 {
                    continue;
                }
                let cc = self.o.return_leaf(cur).coords;
                for ei in 0..12 {
                    let start_node = EDGE_NODES[ei][0];
                    let start = self.node_label(cur, start_node);
                    let end = self.node_label(cur, EDGE_NODES[ei][1]);
                    if start < 0 || end < 0 {
                        continue;
                    }
                    let (s, e) = (start as usize, end as usize);
                    if !(node_level[s] == level && node_level[e] == level) {
                        continue;
                    }
                    let mut create = true;
                    let fi = 2 * (ei / 4) + 1;
                    let f_nodes = FACE_NODES[fi];
                    let edge_cubes: [Label; 4] = std::array::from_fn(|i| self.node_leaf(s, f_nodes[i]));
                    let mut centre_nodes: Vec<Label> = Vec::with_capacity(8);
                    for i in 0..4 {
                        let cl = edge_cubes[i];
                        if cl == -1 || self.cube_label[cl as usize] == -1 {
                            centre_nodes.push(-1);
                            continue;
                        }
                        let cl = cl as usize;
                        let oc = self.o.return_leaf(cl).coords;
                        if oc.level == level {
                            if cl < cur {
                                create = false;
                                break;
                            }
                            centre_nodes.push(self.cube_label[cl]);
                            if let Some(r) = self.face_centre_label[cl] {
                                let fcl = r[FACE_CENTRE_HELPER[ei / 4][i]];
                                if fcl != -1 {
                                    centre_nodes.push(fcl);
                                }
                            }
                        } else if oc.level < level {
                            let sc = cc.plus(&vl_pos[start_node][f_nodes[i]]);
                            match (0..8).find(|&j| sc == oc.refine_for_position(j)) {
                                Some(pos) => {
                                    let sn = self.sub_node(cl as Label, pos);
                                    centre_nodes.push(sn);
                                }
                                None => {
                                    self.defects.missing_cube_positions += 1;
                                    centre_nodes.push(-1);
                                }
                            }
                        }
                    }
                    if !create {
                        continue;
                    }
                    let n = centre_nodes.len();
                    for i in 0..n {
                        if centre_nodes[i] == -1 || centre_nodes[(i + 1) % n] == -1 {
                            continue;
                        }
                        // upstream appends without checkAndAppendTet; a -1
                        // here would be an invalid tet (module docs)
                        let t = [centre_nodes[i], centre_nodes[(i + 1) % n], start, end];
                        if t.iter().any(|&v| v < 0) {
                            self.defects.invalid_edge_tets += 1;
                        } else {
                            self.tets.push(t);
                        }
                    }
                }
            }
        }
    }

    /// `createTetsFromSplitFaces`.
    fn create_tets_from_split_faces(&mut self) {
        for point in 0..self.a.node_leaves.len() {
            for i in 0..6 {
                let f_nodes = FACE_NODES[i];
                let cl = self.node_leaf(point, f_nodes[0]);
                if cl < 0 || self.cube_label[cl as usize] < 0 {
                    continue;
                }
                if (1..4).all(|k| self.node_leaf(point, f_nodes[k]) == cl) {
                    for j in 0..4 {
                        let t = [point as Label, self.sub_node(cl, 7 - f_nodes[j]), self.sub_node(cl, 7 - f_nodes[(j + 1) % 4]), self.cube_label[cl as usize]];
                        self.check_and_append_tet(t);
                    }
                }
            }
        }
    }
}

/// `partTet::mag`-style signed volume `((b − a) × (c − a))·(d − a) / 6`.
pub fn tet_volume(p: &[Vec3], t: &[usize; 4]) -> f64 {
    let (a, b, c, d) = (p[t[0]], p[t[1]], p[t[2]], p[t[3]]);
    b.sub(a).cross(c.sub(a)).dot(d.sub(a)) / 6.0
}

// Tests: `pipeline::tet_mesh::tests::tet_creator_on_the_sphere_and_the_pipe`.
