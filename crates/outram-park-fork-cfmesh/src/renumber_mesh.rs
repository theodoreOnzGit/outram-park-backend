// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/meshes/polyMeshGenModifier/
//     polyMeshGenModifierRenumberMesh.C
//   meshLibrary/utilities/meshes/polyMeshGenAddressing/
//     polyMeshGenAddressingCellCells.C (calcCellCells)
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

//! cfMesh's **`polyMeshGenModifier::renumberMesh`** (gh:#1272), the last
//! step of the `tetMesh` and `pMesh` workflows.
//!
//! 1. A new cell order from a walk over `cellCells` (each cell's neighbours
//!    in the order of its faces, no repeats). Upstream's comment calls it
//!    Cuthill–McKee, but `LongList::remove()` pops the **last** element, so
//!    the walk is depth-first, not breadth-first. The port keeps the stack
//!    (upstream is the specification) and says so here.
//! 2. Internal faces numbered cell by cell in the new order, each cell's in
//!    increasing order of the other cell; boundary faces after them in their
//!    old order (so patches keep their faces and ranges).
//! 3. A face whose new owner would be the higher cell is reversed: the
//!    result is upper-triangular.
//!
//! **Where the port differs.** Upstream reads each cell's faces in the order
//! `polyMeshGen` stores them. [`renumber_mesh`] takes them from the caller
//! (the four workflows pass `VolumeMesh::cell_face_lists`, the carried lists
//! ~~(gh:#1385: every stage rebuilt the mesh from face lists, so none were
//! carried)~~); [`cells_from_mesh`] gives each cell its faces in ascending
//! face order, for a mesh that does not carry them.

use crate::volume_mesh::VolumeMesh;

/// Each cell's faces in ascending face order.
pub fn cells_from_mesh(mesh: &VolumeMesh) -> Vec<Vec<usize>> {
    let mut cells = vec![Vec::new(); mesh.n_cells];
    for f in 0..mesh.face_count() {
        cells[mesh.owner[f]].push(f);
        if let Some(n) = mesh.neighbour[f] {
            cells[n].push(f);
        }
    }
    cells
}

/// The result of [`renumber_mesh`].
#[derive(Debug, Clone)]
pub struct Renumbered {
    /// The renumbered mesh.
    pub mesh: VolumeMesh,
    /// The renumbered cells' face lists.
    pub cells: Vec<Vec<usize>>,
    /// `newOrder`: the old label of each new cell.
    pub new_order: Vec<usize>,
}

/// `polyMeshGenModifier::renumberMesh` (module docs). `cells` are the face
/// lists of the cells of `mesh`, in the order upstream would hold them.
pub fn renumber_mesh(mesh: &VolumeMesh, cells: &[Vec<usize>]) -> Renumbered {
    let nc = mesh.n_cells;
    // calcCellCells
    let cell_cells: Vec<Vec<usize>> = (0..nc)
        .map(|c| {
            let mut nb: Vec<usize> = Vec::new();
            for &f in &cells[c] {
                let mut n = mesh.owner[f];
                if n == c {
                    if let Some(x) = mesh.neighbour[f] {
                        n = x;
                    }
                }
                if n != c && !nb.contains(&n) {
                    nb.push(n);
                }
            }
            nb
        })
        .collect();
    // the walk (LongList::remove pops the last element)
    let mut new_order = Vec::with_capacity(nc);
    let mut visited = vec![false; nc];
    let mut next: Vec<usize> = Vec::new();
    for c0 in 0..nc {
        if visited[c0] {
            continue;
        }
        next.push(c0);
        while let Some(cur) = next.pop() {
            if !visited[cur] {
                visited[cur] = true;
                new_order.push(cur);
                for &n in &cell_cells[cur] {
                    if !visited[n] {
                        next.push(n);
                    }
                }
            }
        }
    }
    reorder_faces(mesh, cells, new_order)
}

/// OpenFOAM's **upper-triangular face order** with the cell numbering kept
/// (gh:#1285): [`reorder_faces`] with the identity cell order. Internal
/// faces end up sorted by (owner, neighbour) with `owner < neighbour`
/// (a face whose owner was the higher cell is reversed, as
/// `face::reverseFace`); boundary faces keep their order, orientation and
/// patches. Points are not touched, so the geometry is the input's; only
/// the face labels (and the winding of reversed faces) move.
///
/// This is step 2–3 of upstream's `renumberMesh` without step 1 (the
/// cell walk). It is what `polyMesh` requires of every mesh it reads
/// (`checkUpperTriangular`) and what `renumberMesh` produces, without
/// changing which label a cell has, so cell zones and per-cell data
/// computed before it stay valid.
pub fn upper_triangular(mesh: &VolumeMesh) -> VolumeMesh {
    let identity: Vec<usize> = (0..mesh.n_cells).collect();
    reorder_faces(mesh, &cells_from_mesh(mesh), identity).mesh
}

/// Steps 2–3 of `renumberMesh` for a given cell order: `new_order[i]` is
/// the old label of new cell `i` (module docs).
pub fn reorder_faces(mesh: &VolumeMesh, cells: &[Vec<usize>], new_order: Vec<usize>) -> Renumbered {
    let nc = mesh.n_cells;
    let nf = mesh.face_count();
    let mut reverse_order = vec![0usize; nc];
    let new_cells: Vec<Vec<usize>> = new_order.iter().map(|&c| cells[c].clone()).collect();
    for (i, &c) in new_order.iter().enumerate() {
        reverse_order[c] = i;
    }
    // -2 internal, -1 boundary
    let mut rfo: Vec<i64> = vec![0; nf];
    for c in &new_cells {
        for &f in c {
            rfo[f] -= 1;
        }
    }
    let mut n_marked: i64 = 0;
    for (ci, c) in new_cells.iter().enumerate() {
        let mut nei_cells: Vec<i64> = vec![-1; c.len()];
        let mut n_nei = 0;
        for (k, &f) in c.iter().enumerate() {
            if rfo[f] == -2 {
                let own = reverse_order[mesh.owner[f]];
                let nb = reverse_order[mesh.neighbour[f].expect("internal face")];
                if ci == own {
                    nei_cells[k] = nb as i64;
                } else if ci == nb {
                    nei_cells[k] = own as i64;
                }
                n_nei += 1;
            }
        }
        for _ in 0..n_nei {
            let mut next_nei = None;
            let mut min_nei = nc as i64;
            for (k, &n) in nei_cells.iter().enumerate() {
                if n > -1 && n < min_nei {
                    next_nei = Some(k);
                    min_nei = n;
                }
            }
            let k = next_nei.expect("internal face insertion");
            rfo[c[k]] = n_marked;
            nei_cells[k] = -1;
            n_marked += 1;
        }
    }
    for x in rfo.iter_mut() {
        if *x < 0 {
            *x = n_marked;
            n_marked += 1;
        }
    }
    let rfo: Vec<usize> = rfo.into_iter().map(|x| x as usize).collect();
    let mut face_order = vec![0usize; nf];
    for (f, &n) in rfo.iter().enumerate() {
        face_order[n] = f;
    }
    let cells_out: Vec<Vec<usize>> = new_cells.iter().map(|c| c.iter().map(|&f| rfo[f]).collect()).collect();
    let mut faces = Vec::with_capacity(nf);
    let mut owner = Vec::with_capacity(nf);
    let mut neighbour = Vec::with_capacity(nf);
    for &old in &face_order {
        let mut ring = mesh.faces[old].clone();
        let o = reverse_order[mesh.owner[old]];
        match mesh.neighbour[old] {
            Some(n) => {
                let n = reverse_order[n];
                if n < o {
                    // face::reverseFace: keep the first point, reverse the rest
                    ring[1..].reverse();
                    owner.push(n);
                    neighbour.push(Some(o));
                } else {
                    owner.push(o);
                    neighbour.push(Some(n));
                }
            }
            None => {
                owner.push(o);
                neighbour.push(None);
            }
        }
        faces.push(ring);
    }
    // boundary faces keep their old relative order, so the patch ranges move
    // by the same shift as the first boundary face
    let n_internal = mesh.n_internal_faces();
    let patches = mesh
        .patches
        .iter()
        .map(|p| {
            let mut q = p.clone();
            if p.n_faces > 0 {
                q.start_face = rfo[p.start_face];
            } else {
                q.start_face = q.start_face.max(n_internal);
            }
            q
        })
        .collect();
    let out = VolumeMesh { points: mesh.points.clone(), faces, owner, neighbour, n_cells: nc, patches, cells: Some(cells_out.clone()) };
    Renumbered { mesh: out, cells: cells_out, new_order }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::shapes::box_surface;
    use crate::math::Vec3;

    /// V&V (gh:#1272): `renumberMesh` on a carved 3 x 2 x 2 block. Checks:
    /// the mesh stays valid with the same volume; owner < neighbour on every
    /// internal face and the internal faces are upper-triangular (sorted by
    /// owner, then neighbour); the patches keep their face sets; cell 0
    /// stays first and the walk is depth-first (the last neighbour pushed
    /// is visited next, `LongList::remove`).
    #[test]
    fn renumber_mesh_is_upper_triangular_and_depth_first() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(1.5, 1.0, 1.0));
        let m = carve_box(&p, &t, 0.5);
        let cells = cells_from_mesh(&m);
        let r = renumber_mesh(&m, &cells);
        r.mesh.validate().unwrap();
        assert!((r.mesh.total_volume() - m.total_volume()).abs() < 1e-12);
        let ni = r.mesh.n_internal_faces();
        let mut prev = (0, 0);
        for f in 0..ni {
            let key = (r.mesh.owner[f], r.mesh.neighbour[f].unwrap());
            assert!(key.0 < key.1);
            assert!(key >= prev, "face {f}: {key:?} after {prev:?}");
            prev = key;
        }
        // patches keep their face sets (as sorted point lists)
        for (a, b) in m.patches.iter().zip(&r.mesh.patches) {
            let set = |mm: &VolumeMesh, pa: &crate::volume_mesh::BoundaryPatch| {
                let mut v: Vec<Vec<usize>> = (pa.start_face..pa.start_face + pa.n_faces)
                    .map(|f| {
                        let mut x = mm.faces[f].clone();
                        x.sort_unstable();
                        x
                    })
                    .collect();
                v.sort();
                v
            };
            assert_eq!(set(&m, a), set(&r.mesh, b));
        }
        assert_eq!(r.new_order[0], 0);
        // depth-first: the second cell is the last neighbour of cell 0
        let cc0: Vec<usize> = cells[0].iter().filter_map(|&f| m.neighbour[f].map(|n| if n == 0 { m.owner[f] } else { n })).collect();
        assert_eq!(r.new_order[1], *cc0.last().unwrap());
        // renumbering is a permutation
        let mut s = r.new_order.clone();
        s.sort_unstable();
        assert_eq!(s, (0..m.n_cells).collect::<Vec<_>>());
    }
}
