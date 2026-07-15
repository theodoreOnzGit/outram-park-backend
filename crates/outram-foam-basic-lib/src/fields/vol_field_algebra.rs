// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
//   src/OpenFOAM/fields/GeometricFields/GeometricField/GeometricFieldFunctions.C
//   src/OpenFOAM/primitives/Tensor/TensorI.H, SymmTensor/SymmTensorI.H
//   @ commit 481094fd (OpenFOAM v2606)
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

//! Field-level (`GeometricField`) tensor algebra.
//!
//! Thin per-element wrappers that lift the primitive `Tensor` / `SymmTensor`
//! operations (`tr`, `symm`, `twoSymm`, `dev`, `dev2` — defined in
//! `crate::primitives`) to whole volume fields, applying the operation to the
//! internal field and every boundary patch and returning the correctly-ranked
//! output field.
//!
//! These belong here (with the field types) rather than in the FV operator
//! layer because they are pure algebra — no mesh metrics, no interpolation.
//! They are the field-level counterparts a `solidDisplacementFoam`-style stress
//! update needs, e.g. `sigma = mu*twoSymm(grad(D)) + lambda*tr(grad(D))*I`.
//!
//! Rank map (OpenFOAM convention):
//!
//! - `tr`        : tensor / symmTensor → scalar
//! - `symm`      : tensor → symmTensor  (0.5·(T + Tᵀ))
//! - `two_symm`  : tensor / symmTensor → symmTensor  (T + Tᵀ)
//! - `dev`       : tensor / symmTensor → same rank   (T − (tr/3)·I)
//! - `dev2`      : tensor / symmTensor → same rank   (T − (2·tr/3)·I)
//!
//! The output boundary patches are set zero-gradient (values carry the mapped
//! result); the operation is applied element-wise so no BC evaluation occurs.

use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::vol_field::{VolField, VolScalarField, VolSymmTensorField, VolTensorField};

/// Map a `VolField<T>` to a `VolField<U>` by applying `f` element-wise to the
/// internal field and to every boundary-patch value.  Output patches are
/// zero-gradient (the mapped values are stored directly).
fn map_vol_field<T, U, F>(vol: &VolField<T>, f: F) -> VolField<U>
where
    T: Clone,
    U: Clone + Default,
    F: Fn(&T) -> U,
{
    let internal = vol.internal.map(&f);
    let boundary = vol
        .boundary
        .iter()
        .map(|pf| PatchField {
            bc: BoundaryCondition::ZeroGradient,
            values: pf.values.map(&f),
        })
        .collect();
    VolField::new(vol.name.clone(), vol.mesh.clone(), internal, boundary)
}

// ── Trace ──────────────────────────────────────────────────────────────────────

/// `tr(T)` of a tensor field → scalar field: `tr = T_xx + T_yy + T_zz`.
pub fn tr(vol: &VolTensorField) -> VolScalarField {
    map_vol_field(vol, |t| t.tr())
}

/// `tr(S)` of a symmetric-tensor field → scalar field.
pub fn tr_of_symm(vol: &VolSymmTensorField) -> VolScalarField {
    map_vol_field(vol, |s| s.tr())
}

// ── Symmetric part ─────────────────────────────────────────────────────────────

/// `symm(T) = 0.5·(T + Tᵀ)` of a tensor field → symmetric-tensor field.
pub fn symm(vol: &VolTensorField) -> VolSymmTensorField {
    map_vol_field(vol, |t| t.symm())
}

/// `twoSymm(T) = T + Tᵀ` of a tensor field → symmetric-tensor field.
pub fn two_symm(vol: &VolTensorField) -> VolSymmTensorField {
    map_vol_field(vol, |t| t.two_symm())
}

/// `twoSymm(S) = 2·S` of a symmetric-tensor field → symmetric-tensor field.
pub fn two_symm_of_symm(vol: &VolSymmTensorField) -> VolSymmTensorField {
    map_vol_field(vol, |s| *s * 2.0)
}

// ── Deviatoric part ────────────────────────────────────────────────────────────

/// `dev(T) = T − (tr(T)/3)·I` of a tensor field → tensor field (trace-free).
pub fn dev(vol: &VolTensorField) -> VolTensorField {
    map_vol_field(vol, |t| t.dev())
}

/// `dev2(T) = T − (2·tr(T)/3)·I` of a tensor field → tensor field.
pub fn dev2(vol: &VolTensorField) -> VolTensorField {
    map_vol_field(vol, |t| t.dev2())
}

/// `dev(S) = S − (tr(S)/3)·I` of a symmetric-tensor field → symmetric-tensor field.
pub fn dev_of_symm(vol: &VolSymmTensorField) -> VolSymmTensorField {
    map_vol_field(vol, |s| s.dev())
}

/// `dev2(S) = S − (2·tr(S)/3)·I` of a symmetric-tensor field → symmetric-tensor field.
pub fn dev2_of_symm(vol: &VolSymmTensorField) -> VolSymmTensorField {
    map_vol_field(vol, |s| s.dev2())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::field::Field;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::primitives::{SymmTensor, Tensor, Vector3};
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
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(0.0, 0.0, 0.0),
                ])
                .build()
                .unwrap(),
        )
    }

    fn tensor_field(m: Arc<crate::mesh::fv_mesh::FvMesh>, t: Tensor) -> VolTensorField {
        // Store `t` on the boundary too so the boundary-mapping is exercised.
        let boundary = m
            .patches
            .iter()
            .map(|p| PatchField::fixed_value_tensor(p.size, t))
            .collect();
        VolTensorField::new("T", m.clone(), Field::uniform(m.n_cells, t), boundary)
    }

    /// V&V — methodology: for T = [[1,2,3],[4,5,6],[7,8,9]] the primitive
    /// identities must hold field-wide (per element, so identical in every cell):
    /// `tr = 15`; `symm` and `two_symm` recover the primitive results;
    /// `symm(T) + skew(T) == T`; `dev` is trace-free; `dev2` has `tr = −tr(T)`.
    ///
    /// Result (measured 2026-07-15): tr = 15; symm.xy = 3 (=(2+4)/2);
    /// two_symm.xy = 6; dev trace < 1e-13; dev2 trace = −15 (< 1e-13 error).
    #[test]
    fn field_algebra_matches_primitive() {
        let m = unit_mesh();
        let t = Tensor::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let vt = tensor_field(m.clone(), t);

        let tr_f = tr(&vt);
        assert!((tr_f.internal[0] - 15.0).abs() < 1e-13);
        // boundary values also mapped
        assert!((tr_f.boundary[0].values[0] - 15.0).abs() < 1e-13);

        let s = symm(&vt);
        assert!((s.internal[0].xy - 3.0).abs() < 1e-13); // (2+4)/2
        assert!((s.internal[0].xz - 5.0).abs() < 1e-13); // (3+7)/2
        assert!((s.internal[0].yz - 7.0).abs() < 1e-13); // (6+8)/2

        let ts = two_symm(&vt);
        assert!((ts.internal[0].xy - 6.0).abs() < 1e-13);
        assert!((ts.internal[0].xx - 2.0).abs() < 1e-13);

        let d = dev(&vt);
        assert!(d.internal[0].tr().abs() < 1e-13, "dev not trace-free");

        let d2 = dev2(&vt);
        assert!((d2.internal[0].tr() - (-15.0)).abs() < 1e-13);
    }

    /// V&V — methodology: symmetric-tensor variants. For
    /// S = symm[xx=2, xy=1, xz=0, yy=4, yz=0, zz=6] (tr = 12):
    /// `tr_of_symm = 12`; `two_symm_of_symm = 2·S`; `dev_of_symm` trace-free;
    /// `dev2_of_symm` trace = −12.
    ///
    /// Result (measured 2026-07-15): tr = 12; two_symm.xx = 4; dev trace < 1e-13;
    /// dev2 trace = −12 (< 1e-13 error).
    #[test]
    fn symm_field_algebra_matches_primitive() {
        let m = unit_mesh();
        let s = SymmTensor::new(2.0, 1.0, 0.0, 4.0, 0.0, 6.0);
        let boundary = m
            .patches
            .iter()
            .map(|p| PatchField::zero_gradient_symm_tensor(p.size))
            .collect();
        let vs = VolSymmTensorField::new("S", m.clone(), Field::uniform(m.n_cells, s), boundary);

        assert!((tr_of_symm(&vs).internal[0] - 12.0).abs() < 1e-13);

        let ts = two_symm_of_symm(&vs);
        assert!((ts.internal[0].xx - 4.0).abs() < 1e-13);
        assert!((ts.internal[0].zz - 12.0).abs() < 1e-13);

        assert!(dev_of_symm(&vs).internal[0].tr().abs() < 1e-13);
        assert!((dev2_of_symm(&vs).internal[0].tr() - (-12.0)).abs() < 1e-13);
    }
}
