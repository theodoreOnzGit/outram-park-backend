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

use crate::fields::vol_field::{VolScalarField, VolVectorField};
use crate::mesh::fv_mesh::FvMesh;
use crate::ldu_matrix::fv_vector_matrix::FvVectorMatrix;

/// Implicit Euler ddt for a `VolVectorField`:
/// `∂U/∂t ≈ (U − U_old) / Δt`
///
/// Assembles:
/// - `diag[c] += V[c] / dt`
/// - `source[c] += V[c] / dt * U_old[c]`
///
/// Mirrors `fvm::ddt(volVectorField, dt)` Euler scheme.
pub fn ddt_vec(
    u: &VolVectorField,
    u_old: &VolVectorField,
    dt: f64,
    mesh: Arc<FvMesh>,
) -> FvVectorMatrix {
    let n = mesh.n_cells;
    let mut mat = FvVectorMatrix::new(mesh.clone());
    for c in 0..n {
        let coeff = mesh.cell_volumes[c] / dt;
        mat.ldu.diag[c] += coeff;
        mat.source[c] = mat.source[c] + u_old.internal[c] * coeff;
    }
    let _ = u; // unused; type ensures correct VolVectorField is passed
    mat
}

/// Field-coefficient implicit Euler ddt for a vector field:
/// `∂(coeff·U)/∂t ≈ coeff·(U − U_old) / Δt`.
///
/// Assembles `coeff[c]·V[c]/Δt` on the diagonal. Covers `fvm::ddt(rho, U)`
/// in the compressible momentum equation.
pub fn ddt_coeff_vec(
    coeff: &VolScalarField,
    u: &VolVectorField,
    u_old: &VolVectorField,
    dt: f64,
    mesh: Arc<FvMesh>,
) -> FvVectorMatrix {
    let n = mesh.n_cells;
    let mut mat = FvVectorMatrix::new(mesh.clone());
    for c in 0..n {
        let c_dt = coeff.internal[c] * mesh.cell_volumes[c] / dt;
        mat.ldu.diag[c] += c_dt;
        mat.source[c] = mat.source[c] + u_old.internal[c] * c_dt;
    }
    let _ = u;
    mat
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::ldu_matrix::fv_matrix::SolverSettings;
    use crate::primitives::Vector3;

    fn unit_mesh() -> Arc<FvMesh> {
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
    fn ddt_vec_recovers_constant_field() {
        let m = unit_mesh();
        let u_old = VolVectorField::uniform("Uold", m.clone(), Vector3::new(1.0, 2.0, 3.0));
        let u     = VolVectorField::uniform("U", m.clone(), Vector3::new(1.0, 2.0, 3.0));
        let mat = ddt_vec(&u, &u_old, 0.1, m);
        let (u_new, perf) = mat.solve("U", SolverSettings::default());
        assert!(perf.converged, "not converged");
        assert!((u_new.internal[0].x - 1.0).abs() < 1e-8);
        assert!((u_new.internal[0].y - 2.0).abs() < 1e-8);
        assert!((u_new.internal[0].z - 3.0).abs() < 1e-8);
    }

    #[test]
    fn ddt_coeff_vec_diagonal_is_coeff_times_volume_over_dt() {
        // rho = 2, V = 1, dt = 0.5  →  diag = 2*1/0.5 = 4
        let m = unit_mesh();
        let rho   = VolScalarField::uniform("rho", m.clone(), 2.0);
        let u_old = VolVectorField::uniform("Uold", m.clone(), Vector3::new(3.0, 0.0, 0.0));
        let u     = VolVectorField::zero("U", m.clone());
        let mat = ddt_coeff_vec(&rho, &u, &u_old, 0.5, m);
        assert!((mat.ldu.diag[0] - 4.0).abs() < 1e-12);
        assert!((mat.ldu.diag[1] - 4.0).abs() < 1e-12);
        // source.x = 4 * 3 = 12
        assert!((mat.source[0].x - 12.0).abs() < 1e-12);
    }

    #[test]
    fn ddt_coeff_vec_solves_to_old_when_no_spatial_terms() {
        let m = unit_mesh();
        let rho   = VolScalarField::uniform("rho", m.clone(), 5.0);
        let u_old = VolVectorField::uniform("Uold", m.clone(), Vector3::new(2.0, -1.0, 4.0));
        let u     = VolVectorField::zero("U", m.clone());
        let mat = ddt_coeff_vec(&rho, &u, &u_old, 1.0, m);
        let (u_new, perf) = mat.solve("U", SolverSettings::default());
        assert!(perf.converged);
        assert!((u_new.internal[0].x - 2.0).abs() < 1e-8);
        assert!((u_new.internal[0].y - (-1.0)).abs() < 1e-8);
        assert!((u_new.internal[0].z - 4.0).abs() < 1e-8);
    }
}
