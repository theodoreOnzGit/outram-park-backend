// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/tetMesh/tetMeshExtractorOctree/tetMeshExtractorOctree.{C,H}
//   meshLibrary/utilities/meshes/polyMeshGen/polyMeshGenCells.C
//     (calculateOwnersAndNeighbours)
//   meshLibrary/utilities/meshes/polyMeshGenModifier/
//     polyMeshGenModifierReorderBoundaryFaces.C,
//     polyMeshGenModifierRemoveUnusedVertices.C
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

//! cfMesh's **`tetMeshExtractorOctree`** (gh:#1272): the tets of
//! [`crate::tet_creator_octree`] as a [`VolumeMesh`].
//!
//! `createPolyMesh` gives every tet its four faces `(a, c, b)`, `(a, b, d)`,
//! `(b, c, d)`, `(c, a, d)` (outward for a positive tet), then removes each
//! face a lower tet already has (the same three points, either winding;
//! OpenFOAM's `face::operator==`), searching only the tets around the face's
//! first point, as upstream. Then the `polyMeshGen` assembly upstream's
//! extractors share, ported here once as [`poly_mesh_from_cells`]:
//!
//! - owner and neighbour of each face are the first and second cell, in
//!   cell order, that list it (`calculateOwnersAndNeighbours`; a third is a
//!   `FatalError` upstream, an `Err` here);
//! - `reorderBoundaryFaces`: a boundary face among the first
//!   `nInternalFaces` swaps place with an internal face beyond them, pair by
//!   pair in ascending order; all boundary faces go to one patch,
//!   `defaultFaces`;
//! - `removeUnusedVertices`: points no face uses are dropped, the others keep
//!   their order.
//!
//! The cell face lists are returned beside the mesh, because upstream's
//! `renumberMesh` reads them in the cells' own order
//! ([`crate::renumber_mesh`]).

use crate::math::Vec3;
use crate::tet_creator_octree::TetCreatorOctree;
use crate::volume_mesh::{BoundaryPatch, VolumeMesh};

/// A mesh assembled from cell face lists, with what upstream keeps beside it.
#[derive(Debug, Clone)]
pub struct AssembledMesh {
    /// The mesh: internal faces first, then one patch `defaultFaces`.
    pub mesh: VolumeMesh,
    /// The faces of each cell, in the cell's own order (`polyMeshGen::cells`).
    pub cells: Vec<Vec<usize>>,
    /// `newLabel` of `removeUnusedVertices`: the new label of each input
    /// point, `None` for a dropped one.
    pub point_map: Vec<Option<usize>>,
}

/// `tetMeshExtractorOctree::createMesh` on the creator's tets (module docs).
pub fn tet_mesh_extractor_octree(tc: &TetCreatorOctree) -> Result<AssembledMesh, String> {
    let tets = &tc.tets;
    let np = tc.tet_points.len();
    // pTets.reverseAddressing(nPoints, tets): ascending tet labels per point
    let mut p_tets: Vec<Vec<usize>> = vec![Vec::new(); np];
    for (t, tet) in tets.iter().enumerate() {
        for &v in tet {
            p_tets[v].push(t);
        }
    }
    let mut faces: Vec<[usize; 3]> = Vec::with_capacity(4 * tets.len());
    let mut cells: Vec<[usize; 4]> = Vec::with_capacity(tets.len());
    for (t, &[a, b, c, d]) in tets.iter().enumerate() {
        faces.push([a, c, b]);
        faces.push([a, b, d]);
        faces.push([b, c, d]);
        faces.push([c, a, d]);
        cells.push([4 * t, 4 * t + 1, 4 * t + 2, 4 * t + 3]);
    }
    let same = |x: &[usize; 3], y: &[usize; 3]| {
        let mut p = *x;
        let mut q = *y;
        p.sort_unstable();
        q.sort_unstable();
        p == q
    };
    let mut remove = vec![false; faces.len()];
    for cell_i in 0..cells.len() {
        for fi in 0..4 {
            let f = faces[cells[cell_i][fi]];
            let point = f[0];
            for &other in &p_tets[point] {
                if other >= cell_i {
                    continue;
                }
                for ofi in 0..4 {
                    let of = cells[other][ofi];
                    if of >= cells[cell_i][fi] {
                        continue;
                    }
                    if same(&f, &faces[of]) {
                        remove[cells[cell_i][fi]] = true;
                        cells[cell_i][fi] = of;
                    }
                }
            }
        }
    }
    let mut new_face = vec![usize::MAX; faces.len()];
    let mut kept: Vec<Vec<usize>> = Vec::new();
    for (f, ring) in faces.iter().enumerate() {
        if !remove[f] {
            new_face[f] = kept.len();
            kept.push(ring.to_vec());
        }
    }
    let cell_faces: Vec<Vec<usize>> = cells.iter().map(|c| c.iter().filter(|&&f| new_face[f] != usize::MAX).map(|&f| new_face[f]).collect()).collect();
    poly_mesh_from_cells(tc.tet_points.clone(), kept, cell_faces)
}

/// The `polyMeshGen` assembly shared by the extractors (module docs):
/// owners and neighbours from the cell face lists, `reorderBoundaryFaces`,
/// `removeUnusedVertices`.
pub fn poly_mesh_from_cells(points: Vec<Vec3>, mut faces: Vec<Vec<usize>>, mut cells: Vec<Vec<usize>>) -> Result<AssembledMesh, String> {
    let nf = faces.len();
    // calculateOwnersAndNeighbours
    let mut own: Vec<Option<usize>> = vec![None; nf];
    let mut nei: Vec<Option<usize>> = vec![None; nf];
    let mut n_internal = 0;
    for (c, cf) in cells.iter().enumerate() {
        for &f in cf {
            if own[f].is_none() {
                own[f] = Some(c);
            } else if nei[f].is_none() {
                nei[f] = Some(c);
                n_internal += 1;
            } else {
                return Err(format!("face {f} appears in more than 2 cells (owner {:?}, neighbour {:?}, cell {c})", own[f], nei[f]));
            }
        }
    }
    if let Some(f) = own.iter().position(|o| o.is_none()) {
        return Err(format!("face {f} belongs to no cell"));
    }
    // reorderBoundaryFaces
    let internal_to_change: Vec<usize> = (0..n_internal).filter(|&f| nei[f].is_none()).collect();
    let boundary_to_change: Vec<usize> = (n_internal..nf).filter(|&f| nei[f].is_some()).collect();
    let mut new_label: Vec<Option<usize>> = vec![None; nf];
    for (&i, &b) in internal_to_change.iter().zip(&boundary_to_change) {
        faces.swap(i, b);
        own.swap(i, b);
        nei.swap(i, b);
        new_label[i] = Some(b);
        new_label[b] = Some(i);
    }
    for c in cells.iter_mut() {
        for f in c.iter_mut() {
            if let Some(n) = new_label[*f] {
                *f = n;
            }
        }
    }
    // removeUnusedVertices
    let mut used = vec![false; points.len()];
    for f in &faces {
        for &p in f {
            used[p] = true;
        }
    }
    let mut point_map = vec![None; points.len()];
    let mut kept_points = Vec::new();
    for (p, &u) in used.iter().enumerate() {
        if u {
            point_map[p] = Some(kept_points.len());
            kept_points.push(points[p]);
        }
    }
    for f in faces.iter_mut() {
        for p in f.iter_mut() {
            *p = point_map[*p].expect("used point");
        }
    }
    let n_cells = cells.len();
    let mesh = VolumeMesh {
        points: kept_points,
        patches: vec![BoundaryPatch { name: "defaultFaces".into(), start_face: n_internal, n_faces: nf - n_internal }],
        faces,
        owner: own.into_iter().map(|o| o.expect("owned")).collect(),
        neighbour: nei,
        n_cells,
        cells: Some(cells.clone()),
    };
    Ok(AssembledMesh { mesh, cells, point_map })
}

/// Bit-for-bit equality of two meshes (points, faces, owners, neighbours,
/// patches). [`VolumeMesh`] does not derive `PartialEq`.
pub fn same_mesh(a: &VolumeMesh, b: &VolumeMesh) -> bool {
    let bits = |m: &VolumeMesh| m.points.iter().map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]).collect::<Vec<_>>();
    bits(a) == bits(b) && a.faces == b.faces && a.owner == b.owner && a.neighbour == b.neighbour && a.n_cells == b.n_cells && a.patches == b.patches
}
