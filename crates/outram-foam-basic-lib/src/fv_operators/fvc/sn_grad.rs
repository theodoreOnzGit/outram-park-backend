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
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::VolScalarField;
use crate::mesh::fv_mesh::PatchKind;

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
                // Cyclic (periodic) seam: normal gradient across the seam to the
                // paired cell, `(φ_paired − φ_owner) / d_seam`.
                if patch.kind == PatchKind::Cyclic {
                    if let Some(pf) = mesh.cyclic_partner_face(gf) {
                        let paired = mesh.owner[pf];
                        let d_b = (mesh.face_centres[pf] - mesh.cell_centres[paired]).mag();
                        let delta = d + d_b;
                        return if delta < 1e-300 {
                            0.0
                        } else {
                            (vol.internal[paired] - vol.internal[owner]) / delta
                        };
                    }
                }
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
                    // No-slip wall: fixedValue of zero.
                    BoundaryCondition::NoSlip => {
                        if d < 1e-300 {
                            0.0
                        } else {
                            (0.0 - vol.internal[owner]) / d
                        }
                    }
                    // fixedGradient: the surface-normal gradient IS the
                    // prescribed value g [value·m⁻¹]. `fixedFluxPressure` is a
                    // fixedGradient whose gradient the solver set (`snGrad(p)`).
                    BoundaryCondition::FixedGradient(g)
                    | BoundaryCondition::FixedFluxPressure { gradient: g } => *g,
                    // totalPressure: Dirichlet using the solver-computed face
                    // value the hook wrote into the patch — snGrad = (p_f − p_c)/d.
                    BoundaryCondition::TotalPressure { .. } => {
                        if d < 1e-300 {
                            0.0
                        } else {
                            (bc_patch.values[fi] - vol.internal[owner]) / d
                        }
                    }
                    // Robin/mixed gradient: w·(refValue − φ_c)/d + (1−w)·refGrad.
                    BoundaryCondition::Mixed {
                        value_fraction,
                        ref_value,
                        ref_grad,
                    } => {
                        let w = *value_fraction;
                        let dirichlet = if d < 1e-300 {
                            0.0
                        } else {
                            (*ref_value - vol.internal[owner]) / d
                        };
                        w * dirichlet + (1.0 - w) * ref_grad
                    }
                    // Zero-gradient-like: Symmetry/Slip/Wedge/Empty and the
                    // flux-switched BCs (no flux available in fvc::snGrad).
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
        crate::fv_operators::naming::derived_name("snGrad", &vol.name),
        vol.mesh.clone(),
        internal,
        boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::vol_field::VolScalarField;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::primitives::Vector3;
    use std::sync::Arc;

    fn unit_mesh() -> Arc<crate::mesh::fv_mesh::FvMesh> {
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

    /// V&V (verification, 2026-08-04). Methodology: a `FixedGradient(g)` patch's
    /// surface-normal gradient must equal the prescribed value g exactly,
    /// independent of the internal field. Set the right patch to
    /// `FixedGradient(3.0)` on the 2-cell unit mesh and read back its boundary
    /// snGrad. Pass criterion: |snGrad − g| < 1e-12.
    /// Result: snGrad = 3.000000 (exact). PASS.
    #[test]
    fn vv_fixed_gradient_reports_prescribed_gradient() {
        use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
        use crate::fields::field::Field;
        let m = unit_mesh();
        let bc = vec![
            PatchField {
                bc: BoundaryCondition::FixedGradient(3.0),
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![0.0]),
            },
        ];
        let t = VolScalarField::new("T", m.clone(), Field::new(vec![7.0, 9.0]), bc);
        let g = sn_grad(&t);
        // patch 0 = "right", one face
        assert!(
            (g.boundary[0].values[0] - 3.0).abs() < 1e-12,
            "snGrad={}",
            g.boundary[0].values[0]
        );
    }

    /// V&V (verification, 2026-08-04). Cyclic-patch surface-normal gradient
    /// crosses the periodic seam. Methodology: on `periodic_1d(4, 1.0, 1.0)`
    /// (h = 0.25) set the field to `[0,1,2,3]`. The left cyclic patch (owner cell
    /// 0, paired cell 3) has seam distance `d_seam = h = 0.25`, so its
    /// surface-normal gradient is `(φ_3 − φ_0)/d_seam = (3 − 0)/0.25 = 12`. Pass
    /// criterion: |snGrad − 12| < 1e-10.
    /// Result: left snGrad = 12.000000. PASS.
    #[test]
    fn vv_cyclic_sn_grad_across_seam() {
        let m = Arc::new(crate::mesh::fv_mesh::FvMesh::periodic_1d(4, 1.0, 1.0));
        let mut t = VolScalarField::zeros("T", m.clone());
        t.internal[0] = 0.0;
        t.internal[1] = 1.0;
        t.internal[2] = 2.0;
        t.internal[3] = 3.0;
        let g = sn_grad(&t);
        assert!(
            (g.boundary[0].values[0] - 12.0).abs() < 1e-10,
            "snGrad={}",
            g.boundary[0].values[0]
        );
    }
}
