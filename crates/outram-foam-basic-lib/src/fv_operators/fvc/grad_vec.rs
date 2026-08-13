// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
//   src/finiteVolume/finiteVolume/fvc/fvcGrad.C
//   src/finiteVolume/finiteVolume/gradSchemes/gaussGrad/gaussGrad.C
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
use crate::fields::vol_field::{VolTensorField, VolVectorField};
use crate::primitives::Tensor;

/// Gauss gradient of a **vector** field: `∇u`, a `VolVectorField → VolTensorField`.
///
/// # Math
///
/// Cell-centred Gauss gradient (finite-volume Green–Gauss theorem):
///
/// `(∇u)_O = (1/V_O) · Σ_f S_f ⊗ u_f`
///
/// where `S_f` is the outward face-area vector, `u_f` the face-interpolated
/// velocity, and `⊗` the outer (dyadic) product.  The resulting tensor uses the
/// OpenFOAM convention in which the **first** index is the derivative direction:
///
/// `(∇u)_{ij} = ∂u_j / ∂x_i`
///
/// so `grad` of a linear field `u = A·x` (i.e. `u_j = A_{jk} x_k`) returns the
/// **constant transpose** `(∇u)_{ij} = A_{ji}`, i.e. `Aᵀ`.
///
/// # Field ranks
///
/// input `VolVectorField` (rank-1) → output `VolTensorField` (rank-2).
///
/// # Discretisation
///
/// Face values `u_f` come from `fvc::interpolate` (linear/geometric weighting on
/// internal faces; BC-evaluated on boundary faces).  The internal-face outer
/// product is accumulated with `+` into the owner cell and `−` into the
/// neighbour cell; boundary faces add only to their owner.  The output boundary
/// is set zero-gradient (extrapolated-calculated in OpenFOAM terms).
///
/// # Assumptions
///
/// Single Gauss pass, no explicit non-orthogonal correction (matches the scalar
/// `fvc::grad` in this crate).  Values carry raw `f64`/`Vector3`/`Tensor`
/// element data — no `uom`, consistent with the rest of the FV operator layer.
pub fn grad_vec(vol: &VolVectorField) -> VolTensorField {
    let mesh = &vol.mesh;
    let u_f = interpolate(vol);

    let mut g = vec![Tensor::ZERO; mesh.n_cells];

    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let n = mesh.neighbour[f];
        // outer product S_f ⊗ u_f : (S_f)_i (u_f)_j  →  T_{ij}
        let contrib = mesh.face_area_vectors[f] * u_f.internal[f];
        g[o] += contrib;
        g[n] -= contrib;
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            let owner = mesh.owner[gf];
            let contrib = mesh.face_area_vectors[gf] * u_f.boundary[pi].values[fi];
            g[owner] += contrib;
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient_tensor(p.size))
        .collect();

    VolTensorField::new(
        crate::fv_operators::naming::derived_name("grad", &vol.name),
        vol.mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| g[c] / mesh.cell_volumes[c]),
        boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::primitives::Vector3;
    use std::sync::Arc;

    /// 1-D two-cell mesh, faces normal to x:
    /// cells at x = 0.25, 0.75 (V = 0.5 each); internal face at x = 0.5;
    /// left boundary at x = 0, right boundary at x = 1.
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

    /// V&V — methodology: a uniform vector field has zero gradient everywhere.
    ///
    /// Result (measured 2026-07-15): all nine components of `∇u` are 0 to
    /// < 1e-12 in both cells.
    #[test]
    fn uniform_field_zero_grad() {
        let m = unit_mesh();
        let u = VolVectorField::uniform("U", m, Vector3::new(3.0, -1.0, 2.0));
        let g = grad_vec(&u);
        for c in 0..2 {
            let t = g.internal[c];
            let max =
                t.xx.abs()
                    .max(t.xy.abs())
                    .max(t.xz.abs())
                    .max(t.yx.abs())
                    .max(t.yy.abs())
                    .max(t.yz.abs())
                    .max(t.zx.abs())
                    .max(t.zy.abs())
                    .max(t.zz.abs());
            assert!(max < 1e-12, "cell {c}: max |grad| = {max:e}");
        }
    }

    /// V&V — methodology: linear vector field `u = A·x` with `u_j = m_j · x`
    /// (m = (2, −3, 5)); Dirichlet BCs at x=0 (u=0) and x=1 (u=m).  The exact
    /// Gauss gradient is the constant tensor `(∇u)_{ij} = ∂u_j/∂x_i`, non-zero
    /// only for i = x, giving row-x = (2, −3, 5) and all other rows 0 — i.e.
    /// `Aᵀ` where A has column-x = m.
    ///
    /// Result (measured 2026-07-15): both cells return
    /// `∇u = [[2, −3, 5], [0,0,0], [0,0,0]]` to < 1e-10.
    #[test]
    fn linear_field_gives_transpose() {
        let m_mesh = unit_mesh();
        let mx = Vector3::new(2.0, -3.0, 5.0);
        // right patch is at x = 1 → u = m·1 = m; left patch at x = 0 → u = 0.
        let boundary = vec![
            PatchField {
                bc: BoundaryCondition::FixedValue(mx),
                values: Field::new(vec![mx]),
            },
            PatchField {
                bc: BoundaryCondition::FixedValue(Vector3::ZERO),
                values: Field::new(vec![Vector3::ZERO]),
            },
        ];
        let mut u = VolVectorField::new(
            "U",
            m_mesh.clone(),
            Field::new(vec![Vector3::ZERO; 2]),
            boundary,
        );
        u.internal[0] = mx * 0.25;
        u.internal[1] = mx * 0.75;

        let g = grad_vec(&u);
        for c in 0..2 {
            let t = g.internal[c];
            assert!((t.xx - 2.0).abs() < 1e-10, "cell {c} xx = {}", t.xx);
            assert!((t.xy - (-3.0)).abs() < 1e-10, "cell {c} xy = {}", t.xy);
            assert!((t.xz - 5.0).abs() < 1e-10, "cell {c} xz = {}", t.xz);
            // No y/z spatial variation on this 1-D mesh.
            assert!(t.yx.abs() < 1e-10 && t.yy.abs() < 1e-10 && t.yz.abs() < 1e-10);
            assert!(t.zx.abs() < 1e-10 && t.zy.abs() < 1e-10 && t.zz.abs() < 1e-10);
        }
    }
}
