// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::field::Field;
use crate::fields::vol_field::VolScalarField;
use crate::fields::surface_field::SurfaceScalarField;

/// Surface-normal gradient: `∂φ/∂n|_f = (φ_N − φ_O) / |C_N − C_O|`.
///
/// Boundary face contributions:
/// - `ZeroGradient` / `Symmetry`: zero normal gradient.
/// - `FixedValue(v)`: `(v − φ_owner) / |C_f − C_owner|`.
pub fn sn_grad(vol: &VolScalarField) -> SurfaceScalarField {
    let mesh = &vol.mesh;

    let internal = Field::from_fn(mesh.n_internal_faces, |f| {
        let o = mesh.owner[f];
        let n = mesh.neighbour[f];
        let delta = (mesh.cell_centres[n] - mesh.cell_centres[o]).mag();
        if delta < 1e-300 {
            return 0.0;
        }
        (vol.internal[n] - vol.internal[o]) / delta
    });

    let boundary = mesh
        .patches
        .iter()
        .zip(vol.boundary.iter())
        .map(|(patch, bc_patch)| {
            let values = Field::from_fn(patch.size, |fi| {
                let gf = patch.start + fi;
                let owner = mesh.owner[gf];
                let d = (mesh.face_centres[gf] - mesh.cell_centres[owner]).mag();
                match &bc_patch.bc {
                    BoundaryCondition::ZeroGradient | BoundaryCondition::Symmetry => 0.0,
                    BoundaryCondition::FixedValue(v) => {
                        if d < 1e-300 { 0.0 } else { (*v - vol.internal[owner]) / d }
                    }
                    BoundaryCondition::FixedField(ff) => {
                        if d < 1e-300 { 0.0 } else { (ff[fi] - vol.internal[owner]) / d }
                    }
                    _ => 0.0,
                }
            });
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values,
            }
        })
        .collect();

    SurfaceScalarField::new(
        format!("snGrad({})", vol.name),
        vol.mesh.clone(),
        internal,
        boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::primitives::Vector3;
    use crate::fields::vol_field::VolScalarField;
    use crate::mesh::fv_mesh::{FvMeshBuilder, BoundaryPatch, PatchKind};

    fn unit_mesh() -> Arc<crate::mesh::fv_mesh::FvMesh> {
        Arc::new(FvMeshBuilder::new()
            .n_cells(2).n_internal_faces(1)
            .owner(vec![0, 1, 0]).neighbour(vec![1])
            .patches(vec![
                BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                BoundaryPatch::new("left",  2, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![1.0, 1.0])
            .cell_centres(vec![Vector3::new(0.25, 0.0, 0.0), Vector3::new(0.75, 0.0, 0.0)])
            .face_area_vectors(vec![
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(-1.0, 0.0, 0.0),
            ])
            .face_centres(vec![
                Vector3::new(0.5, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 0.0),
            ])
            .build().unwrap())
    }

    #[test]
    fn uniform_field_zero_gradient() {
        let m = unit_mesh();
        let p = VolScalarField::uniform("p", m, 3.0);
        let g = sn_grad(&p);
        assert!(g.internal[0].abs() < 1e-12);
    }

    #[test]
    fn linear_field_constant_gradient() {
        // T[0]=0, T[1]=1; |delta| = 0.5 → gradient = 2.0
        let m = unit_mesh();
        let mut t = VolScalarField::zeros("T", m.clone());
        t.internal[1] = 1.0;
        let g = sn_grad(&t);
        assert!((g.internal[0] - 2.0).abs() < 1e-10);
    }
}
