// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
//   src/finiteVolume/finiteVolume/fvc/fvcDiv.C
//   src/finiteVolume/finiteVolume/divSchemes/gaussDivScheme/gaussDivScheme.C
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

use super::interpolate;
use crate::fields::boundary::bc::PatchField;
use crate::fields::field::Field;
use crate::fields::vol_field::{VolSymmTensorField, VolTensorField, VolVectorField};
use crate::primitives::{Tensor, Vector3};

/// Gauss divergence of a **tensor** field: `∇·σ`, a `VolTensorField → VolVectorField`.
///
/// # Math
///
/// Cell-centred Gauss divergence (finite-volume Green–Gauss theorem):
///
/// `(∇·σ)_O = (1/V_O) · Σ_f S_f · σ_f`
///
/// where `S_f` is the outward face-area vector and `σ_f` the face-interpolated
/// tensor.  The face flux `S_f · σ_f` is the vector–tensor inner product with
/// component `j = Σ_i (S_f)_i (σ_f)_{ij}`, i.e. the discrete `∂σ_{ij}/∂x_i`
/// (OpenFOAM convention: contraction is on the **first** tensor index).
///
/// # Field ranks
///
/// input `VolTensorField` (rank-2) → output `VolVectorField` (rank-1).
///
/// # Discretisation
///
/// Face values `σ_f` come from `fvc::interpolate` (linear weighting on internal
/// faces; BC-evaluated on boundary faces).  Internal-face flux is added to the
/// owner and subtracted from the neighbour; boundary faces add to their owner.
/// Output boundary is set zero-gradient.  This is the term behind the
/// `divSigmaExp = fvc::div(σ)` explicit stress divergence of a
/// `solidDisplacementFoam`-style `DEqn`.
pub fn div_tensor(sigma: &VolTensorField) -> VolVectorField {
    let mesh = &sigma.mesh;
    let s_f = interpolate(sigma);
    let mut d = vec![Vector3::ZERO; mesh.n_cells];

    for f in 0..mesh.n_internal_faces {
        // S_f · σ_f  (contraction on first index): component j = Σ_i S_i σ_ij
        let flux = Tensor::vec_mat(mesh.face_area_vectors[f], s_f.internal[f]);
        d[mesh.owner[f]] += flux;
        d[mesh.neighbour[f]] -= flux;
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            let flux = Tensor::vec_mat(mesh.face_area_vectors[gf], s_f.boundary[pi].values[fi]);
            d[mesh.owner[gf]] += flux;
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient_vec(p.size))
        .collect();

    VolVectorField::new(
        format!("div({})", sigma.name),
        sigma.mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| d[c] * (1.0 / mesh.cell_volumes[c])),
        boundary,
    )
}

/// Gauss divergence of a **symmetric-tensor** field: `∇·σ`,
/// a `VolSymmTensorField → VolVectorField`.
///
/// Identical discretisation to [`div_tensor`], but the face flux uses the
/// symmetric-tensor inner product `S_f · σ_f = σ_f · S_f` (for a symmetric
/// tensor the two contractions coincide), component `j = Σ_i (S_f)_i (σ_f)_{ij}`.
///
/// # Field ranks
///
/// input `VolSymmTensorField` (rank-2 symmetric) → output `VolVectorField`.
pub fn div_symm_tensor(sigma: &VolSymmTensorField) -> VolVectorField {
    let mesh = &sigma.mesh;
    let s_f = interpolate(sigma);
    let mut d = vec![Vector3::ZERO; mesh.n_cells];

    for f in 0..mesh.n_internal_faces {
        // σ symmetric ⇒ S_f · σ = σ · S_f = mat_vec(σ, S_f)
        let flux = s_f.internal[f].mat_vec(mesh.face_area_vectors[f]);
        d[mesh.owner[f]] += flux;
        d[mesh.neighbour[f]] -= flux;
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            let flux = s_f.boundary[pi].values[fi].mat_vec(mesh.face_area_vectors[gf]);
            d[mesh.owner[gf]] += flux;
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient_vec(p.size))
        .collect();

    VolVectorField::new(
        format!("div({})", sigma.name),
        sigma.mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| d[c] * (1.0 / mesh.cell_volumes[c])),
        boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::primitives::{SymmTensor, Vector3};
    use std::sync::Arc;

    /// 1-D two-cell mesh (see `grad_vec` tests for geometry).
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

    /// V&V — methodology: linear tensor field `σ_{ij}(x) = M_{ij} · x`; Dirichlet
    /// BCs at x=0 (σ=0) and x=1 (σ=M).  The exact divergence is
    /// `(∇·σ)_j = ∂σ_{ij}/∂x_i = M_{xj}` (only the x-derivative survives on this
    /// 1-D mesh), i.e. the x-row of M.
    ///
    /// M = [[1,2,3],[4,5,6],[7,8,9]] ⇒ expected div = (1, 2, 3) in both cells.
    ///
    /// Result (measured 2026-07-15): both cells return (1, 2, 3) to < 1e-10.
    #[test]
    fn linear_tensor_div_is_x_row() {
        let mesh = unit_mesh();
        let m_t = Tensor::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let boundary = vec![
            PatchField {
                bc: BoundaryCondition::FixedValue(m_t), // x = 1 → σ = M
                values: Field::new(vec![m_t]),
            },
            PatchField {
                bc: BoundaryCondition::FixedValue(Tensor::ZERO), // x = 0 → σ = 0
                values: Field::new(vec![Tensor::ZERO]),
            },
        ];
        let mut sigma = VolTensorField::new(
            "sigma",
            mesh.clone(),
            Field::new(vec![Tensor::ZERO; 2]),
            boundary,
        );
        sigma.internal[0] = m_t * 0.25;
        sigma.internal[1] = m_t * 0.75;

        let d = div_tensor(&sigma);
        for c in 0..2 {
            assert!(
                (d.internal[c].x - 1.0).abs() < 1e-10,
                "cell {c} x = {}",
                d.internal[c].x
            );
            assert!(
                (d.internal[c].y - 2.0).abs() < 1e-10,
                "cell {c} y = {}",
                d.internal[c].y
            );
            assert!(
                (d.internal[c].z - 3.0).abs() < 1e-10,
                "cell {c} z = {}",
                d.internal[c].z
            );
        }
    }

    /// V&V — methodology: same construction for a symmetric tensor
    /// `σ_{ij}(x) = M_{ij}·x`, M = symm[xx=1, xy=2, xz=3, yy=4, yz=5, zz=6].
    /// Expected `(∇·σ)_j = M_{xj} = (xx, xy, xz) = (1, 2, 3)`.
    ///
    /// Result (measured 2026-07-15): both cells return (1, 2, 3) to < 1e-10.
    #[test]
    fn linear_symm_tensor_div_is_x_row() {
        let mesh = unit_mesh();
        let m_s = SymmTensor::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        let boundary = vec![
            PatchField {
                bc: BoundaryCondition::FixedValue(m_s),
                values: Field::new(vec![m_s]),
            },
            PatchField {
                bc: BoundaryCondition::FixedValue(SymmTensor::ZERO),
                values: Field::new(vec![SymmTensor::ZERO]),
            },
        ];
        let mut sigma = VolSymmTensorField::new(
            "sigma",
            mesh.clone(),
            Field::new(vec![SymmTensor::ZERO; 2]),
            boundary,
        );
        sigma.internal[0] = m_s * 0.25;
        sigma.internal[1] = m_s * 0.75;

        let d = div_symm_tensor(&sigma);
        for c in 0..2 {
            assert!(
                (d.internal[c].x - 1.0).abs() < 1e-10,
                "cell {c} x = {}",
                d.internal[c].x
            );
            assert!(
                (d.internal[c].y - 2.0).abs() < 1e-10,
                "cell {c} y = {}",
                d.internal[c].y
            );
            assert!(
                (d.internal[c].z - 3.0).abs() < 1e-10,
                "cell {c} z = {}",
                d.internal[c].z
            );
        }
    }
}
