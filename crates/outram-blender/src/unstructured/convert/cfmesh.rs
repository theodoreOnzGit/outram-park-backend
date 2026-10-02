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

//! Orchestration of `outram-park-fork-cfmesh` (feature `foam-mesh`): the
//! tet -> dual -> boundary-layer pipeline already bridged by
//! [`crate::foam_mesh`], with its `VolumeMesh` handed on to the neutral mesh.
//!
//! cfMesh patches carry a name only; they arrive as [`PatchKind::Patch`] and
//! can be retyped with
//! [`UnstructuredMesh::with_patch_where`](crate::unstructured::UnstructuredMesh::with_patch_where).

use outram_park_fork_cfmesh::pipeline::{TetDualOptions, TetDualReport};
use outram_park_fork_cfmesh::volume_mesh::VolumeMesh;

use super::ConvertError;
use crate::mesh::Mesh;
use crate::unstructured::element::{LengthUnit, Patch, PatchKind, PolyFace};
use crate::unstructured::mesh::UnstructuredMesh;

/// cfMesh `VolumeMesh` (metres) -> neutral mesh (metres).
///
/// # Errors
/// Any construction error of
/// [`UnstructuredMesh::from_polyhedral`] — in particular a dual cell that is
/// not star-shaped about its centroid is refused rather than silently
/// mis-located.
pub fn from_volume_mesh(vm: &VolumeMesh) -> Result<UnstructuredMesh, ConvertError> {
    let points = vm.points.iter().map(|p| [p.x, p.y, p.z]).collect();
    let faces = vm
        .faces
        .iter()
        .enumerate()
        .map(|(f, verts)| PolyFace { verts: verts.clone(), owner: vm.owner[f], neighbour: vm.neighbour[f] })
        .collect();
    let patches = vm
        .patches
        .iter()
        .map(|p| Patch {
            name: p.name.clone(),
            kind: PatchKind::Patch,
            faces: (p.start_face..p.start_face + p.n_faces).collect(),
        })
        .collect();
    Ok(UnstructuredMesh::from_polyhedral(LengthUnit::Metre, points, faces, vm.n_cells, patches, Vec::new())?)
}

/// Mesh a closed blender surface with cfMesh's tet -> dual -> layers
/// pipeline ([`crate::foam_mesh::mesh_to_tet_dual`]) and return the neutral
/// mesh with cfMesh's own report.
///
/// # Errors
/// [`ConvertError::Solver`] with the pipeline's message (a leaky or
/// degenerate surface is refused before meshing), or a neutral-mesh error.
pub fn tet_dual(surface: &Mesh, opts: &TetDualOptions) -> Result<(UnstructuredMesh, TetDualReport), ConvertError> {
    let (vm, report) = crate::foam_mesh::mesh_to_tet_dual(surface, opts).map_err(ConvertError::Solver)?;
    Ok((from_volume_mesh(&vm)?, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// A 2 m cube through cfMesh (the same case as `foam_mesh`'s headline
    /// V&V): the neutral mesh keeps cfMesh's cell count and its 8 m^3.
    #[test]
    fn cube_tet_dual_reaches_the_neutral_mesh() {
        let opts = TetDualOptions { cell_size: 0.5, first_layer_thickness: 0.02, ..Default::default() };
        let (vm, report) = crate::foam_mesh::mesh_to_tet_dual(&primitives::cube(2.0), &opts).unwrap();
        let m = from_volume_mesh(&vm).unwrap();
        assert_eq!(m.n_cells(), vm.n_cells);
        assert!((m.total_volume() - report.total_volume).abs() < 1e-9 * report.total_volume);
    }
}
