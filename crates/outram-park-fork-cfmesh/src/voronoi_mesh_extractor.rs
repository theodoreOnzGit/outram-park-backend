// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/voronoiMesh/voronoiMeshExtractor/
//     voronoiMeshExtractor.{C,H}, voronoiMeshExtractorAddressing.C,
//     voronoiMeshExtractorCreateMesh.C
//   meshLibrary/utilities/meshes/primitives/partTet/partTetI.H (edges,
//     centroid, whichPosition)
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

//! cfMesh's **`voronoiMeshExtractor`** (gh:#1273): the polyhedral mesh of
//! `pMesh`, the dual of the [`crate::tet_creator_octree`] tets.
//!
//! - Points: the centroid (vertex average, `partTet::centroid`) of every
//!   tet.
//! - Edges: each tet edge once, stored oriented as in the lowest tet that
//!   has it (`partTet::edges` order), with the tets around it.
//! - The tets around an edge are put in circular order: for each tet in
//!   turn, the point opposite across the face shared with the next tet is
//!   found from upstream's `sameOrientation_` / `oppositeOrientation_`
//!   tables, and the tet holding it is swapped into the next place. An edge
//!   where one tet has no such neighbour is a **boundary edge**.
//! - Cells: one per tet point with at least one edge and no boundary edge.
//! - Faces: one per non-boundary edge with a cell at either end, the ring of
//!   its tets' centroids, wound from `e[0]`'s cell to `e[1]`'s (reversed
//!   when `e[0]` has no cell or has the higher cell label); a face with a
//!   cell at only one end is a boundary face.
//!
//! Then the shared `polyMeshGen` assembly
//! ([`crate::tet_mesh_extractor_octree::poly_mesh_from_cells`]).
//!
//! It is the crate's [`crate::cell_centre_dual`] (OpenFOAM `polyDualMesh`)
//! without the boundary-vertex cells: the interior cells have the same
//! corners (compared in the tests).

use crate::tet_creator_octree::TetCreatorOctree;
use crate::tet_mesh_extractor_octree::{poly_mesh_from_cells, AssembledMesh};

/// `voronoiMeshExtractor::sameOrientation_`.
const SAME_ORIENTATION: [usize; 6] = [3, 1, 2, 2, 3, 0];
/// `voronoiMeshExtractor::oppositeOrientation_`.
const OPPOSITE_ORIENTATION: [usize; 6] = [2, 3, 1, 0, 0, 1];

/// `partTet::edges`.
fn tet_edges(t: &[usize; 4]) -> [[usize; 2]; 6] {
    [[t[0], t[1]], [t[0], t[2]], [t[0], t[3]], [t[3], t[1]], [t[1], t[2]], [t[3], t[2]]]
}

/// The Voronoi mesh and the tet point of each cell.
#[derive(Debug, Clone)]
pub struct VoronoiMesh {
    /// The mesh (points are the used tet centroids).
    pub mesh: crate::volume_mesh::VolumeMesh,
    /// The cells' face lists.
    pub cells: Vec<Vec<usize>>,
    /// `cellLabel`: the cell of each tet point, `None` for none.
    pub cell_of_tet_point: Vec<Option<usize>>,
    /// Edges of the tetrahedrisation.
    pub n_edges: usize,
    /// Of them, boundary edges.
    pub n_boundary_edges: usize,
}

/// `voronoiMeshExtractor::createMesh` on the creator's tets (module docs).
pub fn voronoi_mesh_extractor(tc: &TetCreatorOctree) -> Result<VoronoiMesh, String> {
    let tets = &tc.tets;
    let np = tc.tet_points.len();
    // pointTets.reverseAddressing(tets)
    let mut point_tets: Vec<Vec<usize>> = vec![Vec::new(); np];
    for (t, tet) in tets.iter().enumerate() {
        for &v in tet {
            point_tets[v].push(t);
        }
    }
    // createAddressing: edges and edgeTets
    let mut edges: Vec<[usize; 2]> = Vec::new();
    let mut edge_tets: Vec<Vec<usize>> = Vec::new();
    for (ti, tet) in tets.iter().enumerate() {
        for e in tet_edges(tet) {
            let end_tets = &point_tets[e[1]];
            let mut store = true;
            let mut e_tets = Vec::new();
            for &tj in &point_tets[e[0]] {
                if !end_tets.contains(&tj) {
                    continue;
                }
                if tj < ti {
                    store = false;
                    break;
                }
                e_tets.push(tj);
            }
            if store {
                edge_tets.push(e_tets);
                edges.push(e);
            }
        }
    }
    let mut point_edges: Vec<Vec<usize>> = vec![Vec::new(); np];
    for (ei, e) in edges.iter().enumerate() {
        point_edges[e[0]].push(ei);
        point_edges[e[1]].push(ei);
    }
    // sort edge-tets in circular order
    let mut boundary_edge = vec![false; edges.len()];
    for ei in 0..edges.len() {
        let e = edges[ei];
        let e_tets = &mut edge_tets[ei];
        for ti in 0..e_tets.len() {
            let pt = tets[e_tets[ti]];
            let mut search = None;
            for (k, te) in tet_edges(&pt).iter().enumerate() {
                let same = (te[0] == e[0] && te[1] == e[1]) || (te[0] == e[1] && te[1] == e[0]);
                if same {
                    search = Some(if te[0] == e[0] { pt[SAME_ORIENTATION[k]] } else { pt[OPPOSITE_ORIENTATION[k]] });
                    break;
                }
            }
            let search = search.ok_or_else(|| format!("invalid search point at edge {ei}"))?;
            let mut found = false;
            for i in 0..e_tets.len() {
                if i == ti {
                    continue;
                }
                if !tets[e_tets[i]].contains(&search) {
                    continue;
                }
                if ti < e_tets.len() - 1 {
                    e_tets.swap(ti + 1, i);
                }
                found = true;
                break;
            }
            if !found {
                boundary_edge[ei] = true;
            }
        }
    }
    // createPoints: tet centroids
    let points: Vec<_> = tets
        .iter()
        .map(|t| {
            let p = &tc.tet_points;
            p[t[0]].add(p[t[1]]).add(p[t[2]]).add(p[t[3]]).scale(1.0 / 4.0)
        })
        .collect();
    // createPolyMesh
    let mut cell_label: Vec<Option<usize>> = vec![None; np];
    let mut n_cells = 0;
    for p in 0..np {
        if point_edges[p].is_empty() || point_edges[p].iter().any(|&e| boundary_edge[e]) {
            continue;
        }
        cell_label[p] = Some(n_cells);
        n_cells += 1;
    }
    let mut faces: Vec<Vec<usize>> = Vec::new();
    let mut cells: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
    for (ei, e) in edges.iter().enumerate() {
        if boundary_edge[ei] {
            continue;
        }
        let (c_own, c_nei) = (cell_label[e[0]], cell_label[e[1]]);
        if c_own.is_none() && c_nei.is_none() {
            continue;
        }
        let mut f = edge_tets[ei].clone();
        let reverse = match (c_own, c_nei) {
            (None, _) => true,
            (Some(o), Some(n)) => o > n,
            _ => false,
        };
        if reverse {
            // face::reverseFace: keep the first point, reverse the rest
            f[1..].reverse();
        }
        let fi = faces.len();
        faces.push(f);
        if let Some(o) = c_own {
            cells[o].push(fi);
        }
        if let Some(n) = c_nei {
            cells[n].push(fi);
        }
    }
    let n_boundary_edges = boundary_edge.iter().filter(|&&b| b).count();
    let AssembledMesh { mesh, cells, .. } = poly_mesh_from_cells(points, faces, cells)?;
    Ok(VoronoiMesh { mesh, cells, cell_of_tet_point: cell_label, n_edges: edges.len(), n_boundary_edges })
}
