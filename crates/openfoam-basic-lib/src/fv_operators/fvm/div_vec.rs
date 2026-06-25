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

use std::sync::Arc;

use crate::fields::boundary::bc::BoundaryCondition;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::VolVectorField;
use crate::mesh::fv_mesh::FvMesh;
use crate::ldu_matrix::fv_vector_matrix::FvVectorMatrix;

/// Implicit upwind convection of a vector field `U` by a face flux `phi`:
/// `∇·(φ U)` (assembles into a `FvVectorMatrix`).
///
/// Sign convention:
/// - `phi[f] ≥ 0` → flux flows from owner to neighbour (upwind = owner):
///   - `diag[owner] += phi[f]`  (implicit term on U[owner])
///   - `diag[nbr]   -= 0`       (no contribution)
///   - `upper[f]    += phi[f].min(0.0) = 0`
/// - `phi[f] < 0`  → flux flows from neighbour to owner (upwind = neighbour):
///   - `upper[f]    += phi[f]`  (implicit off-diagonal)
///   - `diag[nbr]   -= phi[f]`
///
/// Mirrors `fvm::div(phi, U)` with the upwind convection scheme.
pub fn div_vec(
    phi: &SurfaceScalarField,
    u: &VolVectorField,
    mesh: Arc<FvMesh>,
) -> FvVectorMatrix {
    let mut mat = FvVectorMatrix::new(mesh.clone());

    // Internal faces
    for f in 0..mesh.n_internal_faces {
        let o  = mesh.owner[f];
        let nb = mesh.neighbour[f];
        let phi_f = phi.internal[f];

        mat.ldu.diag[o]  += phi_f.max(0.0);
        mat.ldu.upper[f] += phi_f.min(0.0);
        mat.ldu.diag[nb] -= phi_f.min(0.0);
        mat.ldu.lower[f] -= phi_f.max(0.0);
    }

    // Boundary faces: explicit contribution (upwind = owner cell)
    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let owner = mesh.owner[patch.start + fi];
            let phi_f = phi.boundary[pi].values[fi];

            match u.boundary[pi].bc {
                BoundaryCondition::FixedValue(ref _v) => {
                    // Known inflow/outflow: add explicit source
                    let u_bc = u.boundary[pi].values[fi];
                    mat.source[owner] = mat.source[owner] - u_bc * phi_f;
                }
                BoundaryCondition::ZeroGradient => {
                    // Upwind = owner: diag contribution
                    mat.ldu.diag[owner] += phi_f;
                }
                _ => {}
            }
        }
    }

    let _ = u;
    mat
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
    use crate::fields::field::Field;
    use crate::fields::surface_field::SurfaceScalarField;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::primitives::Vector3;

    fn two_cell_mesh() -> Arc<FvMesh> {
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
    fn zero_flux_gives_zero_matrix() {
        let m = two_cell_mesh();
        let u = VolVectorField::uniform("U", m.clone(), Vector3::new(1.0, 0.0, 0.0));
        let phi_bnd: Vec<_> = m.patches.iter()
            .map(|p| PatchField { bc: BoundaryCondition::ZeroGradient, values: Field::uniform(p.size, 0.0) })
            .collect();
        let phi = SurfaceScalarField::new("phi", m.clone(), Field::uniform(1, 0.0), phi_bnd);
        let mat = div_vec(&phi, &u, m);
        assert!(mat.ldu.diag.iter().all(|&d| d == 0.0));
        assert!(mat.ldu.upper.iter().all(|&d| d == 0.0));
    }
}
