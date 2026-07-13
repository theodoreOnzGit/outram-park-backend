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

use crate::fields::boundary::bc::BoundaryCondition;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::VolScalarField;
use crate::ldu_matrix::fv_matrix::FvMatrix;

/// Implicit first-order upwind convection: assembles the matrix for `∇·(φ·ψ)`.
///
/// `phi` is the face flux field (SurfaceScalarField); `psi` is the transported
/// scalar (VolScalarField). The upwind scheme selects the donor cell:
///
/// - `φ_f ≥ 0`: flux comes from the **owner** cell → coefficient on `diag[O]`.
/// - `φ_f < 0`: flux comes from the **neighbour** cell → coefficient on `upper[f]`.
///
/// ## Boundary conditions
///
/// - `ZeroGradient` / `Symmetry`: boundary value equals owner cell → flux goes
///   entirely to `diag[owner]` regardless of sign.
/// - `FixedValue(v)`: inflow (`φ_f < 0`) uses the fixed value → explicit source;
///   outflow (`φ_f ≥ 0`) remains on the diagonal (upwind from owner).
pub fn div(phi: &SurfaceScalarField, psi: &VolScalarField) -> FvMatrix {
    let mesh = psi.mesh.clone();
    let mut mat = FvMatrix::new(mesh.clone());

    // Internal faces: upwind
    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let n = mesh.neighbour[f];
        let phi_f = phi.internal[f];

        // Owner row O: outflow contributes diag, inflow contributes upper (N column)
        mat.ldu.diag[o] += phi_f.max(0.0);
        mat.ldu.upper[f] += phi_f.min(0.0);

        // Neighbour row N: inflow from O contributes diag, outflow contributes lower (O column)
        mat.ldu.diag[n] -= phi_f.min(0.0);
        mat.ldu.lower[f] -= phi_f.max(0.0);
    }

    // Boundary faces
    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let owner = mesh.owner[patch.start + fi];
            let phi_f = phi.boundary[pi].values[fi];
            match &psi.boundary[pi].bc {
                BoundaryCondition::ZeroGradient | BoundaryCondition::Symmetry => {
                    // psi_face = psi_owner (zero gradient) → always on diagonal
                    mat.ldu.diag[owner] += phi_f;
                }
                BoundaryCondition::FixedValue(v) => {
                    if phi_f >= 0.0 {
                        // Outflow: upwind donor is owner cell
                        mat.ldu.diag[owner] += phi_f;
                    } else {
                        // Inflow: known boundary value → explicit
                        mat.source[owner] -= phi_f * v;
                    }
                }
                BoundaryCondition::FixedField(ff) => {
                    if phi_f >= 0.0 {
                        mat.ldu.diag[owner] += phi_f;
                    } else {
                        mat.source[owner] -= phi_f * ff[fi];
                    }
                }
                _ => {
                    if phi_f >= 0.0 {
                        mat.ldu.diag[owner] += phi_f;
                    }
                }
            }
        }
    }

    mat
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::primitives::Vector3;
    use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
    use crate::fields::field::Field;
    use crate::fields::surface_field::SurfaceScalarField;
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
            .cell_volumes(vec![0.5, 0.5])
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

    fn make_phi(m: Arc<crate::mesh::fv_mesh::FvMesh>, internal: f64, bnd: f64) -> SurfaceScalarField {
        let n_int = m.n_internal_faces;
        let bnd_vals: Vec<_> = m.patches.iter()
            .map(|p| PatchField { bc: BoundaryCondition::ZeroGradient, values: Field::uniform(p.size, bnd) })
            .collect();
        SurfaceScalarField::new("phi", m, Field::uniform(n_int, internal), bnd_vals)
    }

    #[test]
    fn upwind_positive_flux_on_diagonal() {
        // phi_f > 0: donor is owner → only diag[O] += phi, upper unchanged
        let m = unit_mesh();
        let phi = make_phi(m.clone(), 1.0, 0.0);
        let psi = VolScalarField::uniform("psi", m.clone(), 0.0);
        let mat = div(&phi, &psi);
        // internal face: diag[0] += 1, lower[0] -= 1; diag[1] -= 0, upper[0] += 0
        assert!((mat.ldu.diag[0] - 1.0).abs() < 1e-12, "diag[0]={}", mat.ldu.diag[0]);
        assert!((mat.ldu.upper[0] - 0.0).abs() < 1e-12);
        assert!((mat.ldu.diag[1] - 0.0).abs() < 1e-12);
        assert!((mat.ldu.lower[0] - (-1.0)).abs() < 1e-12, "lower[0]={}", mat.ldu.lower[0]);
    }

    #[test]
    fn upwind_negative_flux_on_upper() {
        // phi_f < 0: donor is neighbour → only upper[f] += phi (negative)
        let m = unit_mesh();
        let phi = make_phi(m.clone(), -1.0, 0.0);
        let psi = VolScalarField::uniform("psi", m.clone(), 0.0);
        let mat = div(&phi, &psi);
        assert!((mat.ldu.upper[0] - (-1.0)).abs() < 1e-12);
        assert!((mat.ldu.diag[0] - 0.0).abs() < 1e-12);
        assert!((mat.ldu.diag[1] - 1.0).abs() < 1e-12);
    }
}
