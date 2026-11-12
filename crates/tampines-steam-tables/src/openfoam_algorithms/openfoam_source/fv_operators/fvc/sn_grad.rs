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

use outram_foam_basic_lib::fv_operators::fvm::DeltaCoeff;
use crate::openfoam_algorithms::openfoam_source::boundary::bc::{BoundaryCondition, PatchField};
use crate::openfoam_algorithms::openfoam_source::field::Field;
use crate::openfoam_algorithms::openfoam_source::vol_field::VolScalarField;
use crate::openfoam_algorithms::openfoam_source::surface_field::SurfaceScalarField;

/// Surface-normal gradient: `∂φ/∂n|_f = (φ_N − φ_O) / |C_N − C_O|`.
///
/// Boundary face contributions:
/// - `ZeroGradient` / `Symmetry`: zero normal gradient.
/// - `FixedValue(v)`: `(v − φ_owner) / |n (n · (C_f − C_owner))|`, upstream's boundary
///   `deltaCoeffs` (~~`/ |C_f − C_owner|`~~ **CORRECTED 2026-11-12, gh:#1494**).
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
                // gh:#1494 (from gh:#1488): ~~`|Cf - C|`~~ **CORRECTED 2026-11-12** patch-normal
                // `fvPatch::delta()` (ESI v2506 `fvPatch.C:156`), via basic-lib's `boundary_delta`.
                let d = DeltaCoeff::Orthogonal.boundary_delta(mesh, gf);
                match &bc_patch.bc {
                    BoundaryCondition::ZeroGradient | BoundaryCondition::Symmetry => 0.0,
                    BoundaryCondition::FixedValue(v) => {
                        if d < 1e-300 {
                            0.0
                        } else {
                            (*v - vol.internal[owner]) / d
                        }
                    }
                    BoundaryCondition::FixedField(ff) => {
                        if d < 1e-300 {
                            0.0
                        } else {
                            (ff[fi] - vol.internal[owner]) / d
                        }
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
    use crate::openfoam_algorithms::openfoam_source::{FvMesh, Vector3};
    use crate::openfoam_algorithms::openfoam_source::vol_field::VolScalarField;
    use crate::openfoam_algorithms::openfoam_source::fv_mesh::{
        FvMeshBuilder, BoundaryPatch, PatchKind,
    };

    fn unit_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(2)
                .n_internal_faces(1)
                .owner(vec![0, 1, 0])
                .neighbour(vec![1])
                .patches(vec![
                    BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                    BoundaryPatch::new("left", 2, 1, PatchKind::Wall),
                ])
                .cell_volumes(vec![1.0, 1.0])
                .cell_centres(vec![
                    Vector3::new(0.25, 0.0, 0.0),
                    Vector3::new(0.75, 0.0, 0.0),
                ])
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
                .build()
                .unwrap(),
        )
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

    /// Two cells like `unit_mesh`, but the RIGHT boundary face centre is shifted
    /// transversally to `(1, 0.25, 0)`: `Cf - C = (0.25, 0.25, 0)` (|d| = 0.3536)
    /// while the patch normal is `+x`, so `fvPatch::delta() = (0.25, 0, 0)`.
    /// gh:#1494.
    fn skewed_boundary_mesh() -> std::sync::Arc<crate::openfoam_algorithms::openfoam_source::FvMesh> {
        use crate::openfoam_algorithms::openfoam_source::fv_mesh::{
            BoundaryPatch, FvMeshBuilder, PatchKind,
        };
        use crate::openfoam_algorithms::openfoam_source::Vector3;
        std::sync::Arc::new(
            FvMeshBuilder::new()
                .n_cells(2)
                .n_internal_faces(1)
                .owner(vec![0, 1, 0])
                .neighbour(vec![1])
                .patches(vec![
                    BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                    BoundaryPatch::new("left", 2, 1, PatchKind::Wall),
                ])
                .cell_volumes(vec![0.5, 0.5])
                .cell_centres(vec![
                    Vector3::new(0.25, 0.0, 0.0),
                    Vector3::new(0.75, 0.0, 0.0),
                ])
                .face_area_vectors(vec![
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(-1.0, 0.0, 0.0),
                ])
                .face_centres(vec![
                    Vector3::new(0.5, 0.0, 0.0),
                    Vector3::new(1.0, 0.25, 0.0),
                    Vector3::new(0.0, 0.0, 0.0),
                ])
                .build()
                .unwrap(),
        )
    }

    #[test]
    fn boundary_sn_grad_uses_the_patch_normal_delta_gh1494() {
        // Right face fixedValue 1, owner cell value 0: (1 - 0)/0.25 = 4
        // (old (1 - 0)/0.35355 = 2.828).
        use crate::openfoam_algorithms::openfoam_source::boundary::bc::{BoundaryCondition, PatchField};
        use crate::openfoam_algorithms::openfoam_source::field::Field;
        let m = skewed_boundary_mesh();
        let bc = vec![
            PatchField {
                bc: BoundaryCondition::FixedValue(1.0),
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![0.0]),
            },
        ];
        let t = VolScalarField::new("T", m.clone(), Field::zeros(2), bc);
        let g = sn_grad(&t);
        assert!((g.boundary[0].values[0] - 4.0).abs() < 1e-12, "{}", g.boundary[0].values[0]);
    }
}
