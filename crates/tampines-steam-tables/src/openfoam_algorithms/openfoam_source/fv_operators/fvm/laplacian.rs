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

use crate::openfoam_algorithms::openfoam_source::boundary::bc::BoundaryCondition;
use crate::openfoam_algorithms::openfoam_source::surface_field::SurfaceScalarField;
use crate::openfoam_algorithms::openfoam_source::vol_field::VolScalarField;
use crate::openfoam_algorithms::openfoam_source::fv_matrix::FvMatrix;

/// Implicit Gauss-orthogonal Laplacian: assembles the matrix for `−∇·(Γ∇φ)`.
///
/// ## Sign convention (matches OpenFOAM)
///
/// The returned matrix has **positive** diagonal and **negative** off-diagonals,
/// so the matrix–vector product `A·φ` approximates `−∇·(Γ∇φ)` — the diffusion
/// term *already carrying its minus sign*. It is therefore **ADDED** to the
/// equation matrix, not subtracted:
///
/// ```text
/// // ∂φ/∂t − ∇·(Γ∇φ) = S
/// let eqn = fvm::ddt(&phi, &phi_old, dt) + fvm::laplacian(&gamma_f, &phi);
/// ```
///
/// **Corrected 2026-08-12.** This example previously showed `-
/// fvm::laplacian(...)`, which is anti-diffusion and would amplify a
/// perturbation instead of smoothing it. Every real caller in this crate already
/// used `+` (the momentum predictor's `fvm::laplacian_vec(&mu, &u)` and the
/// energy equation's `fvm::laplacian(&alpha_h_f, &he)`), and the sign is now
/// pinned by a test against a closed-form reference — see
/// `rhoPimpleFoam::lateral_coupling::tests::axial_conduction_matches_analytical_fourier_decay`,
/// which measures the decay of a Fourier mode at +0.237 % of the analytical
/// `exp(−a k² t)` and fails outright if the term is anti-diffusive. The same
/// wording is likely still present in the `outram-foam-basic-lib` copy this file
/// was vendored from.
///
/// ## Boundary conditions
///
/// - `ZeroGradient` / `Symmetry`: no contribution (zero normal flux).
/// - `FixedValue(v)`: adds `coeff` to diagonal and `coeff·v` to source.
pub fn laplacian(gamma: &SurfaceScalarField, phi: &VolScalarField) -> FvMatrix {
    let mesh = phi.mesh.clone();
    let mut mat = FvMatrix::new(mesh.clone());

    // Internal faces: Gauss orthogonal
    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let n = mesh.neighbour[f];
        let delta = (mesh.cell_centres[n] - mesh.cell_centres[o]).mag();
        if delta < 1e-300 {
            continue;
        }
        let coeff = gamma.internal[f] * mesh.face_areas[f] / delta;
        mat.ldu.diag[o] += coeff;
        mat.ldu.diag[n] += coeff;
        mat.ldu.upper[f] = -coeff;
        mat.ldu.lower[f] = -coeff;
    }

    // Boundary faces
    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            let owner = mesh.owner[gf];
            let d = (mesh.face_centres[gf] - mesh.cell_centres[owner]).mag();
            if d < 1e-300 {
                continue;
            }
            let coeff = gamma.boundary[pi].values[fi] * mesh.face_areas[gf] / d;
            match &phi.boundary[pi].bc {
                BoundaryCondition::ZeroGradient | BoundaryCondition::Symmetry => {}
                BoundaryCondition::FixedValue(v) => {
                    mat.ldu.diag[owner] += coeff;
                    mat.source[owner] += coeff * v;
                }
                BoundaryCondition::FixedField(ff) => {
                    mat.ldu.diag[owner] += coeff;
                    mat.source[owner] += coeff * ff[fi];
                }
                _ => {}
            }
        }
    }

    mat
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::openfoam_algorithms::openfoam_source::{FvMesh, SolverSettings, Vector3};
    use crate::openfoam_algorithms::openfoam_source::boundary::bc::{BoundaryCondition, PatchField};
    use crate::openfoam_algorithms::openfoam_source::field::Field;
    use crate::openfoam_algorithms::openfoam_source::surface_field::SurfaceScalarField;
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

    fn uniform_gamma(m: Arc<FvMesh>, val: f64) -> SurfaceScalarField {
        let _n_faces = m.owner.len();
        let internal = Field::uniform(m.n_internal_faces, val);
        let boundary = m
            .patches
            .iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::uniform(p.size, val),
            })
            .collect();
        SurfaceScalarField::new("gamma", m, internal, boundary)
    }

    #[test]
    fn laplacian_symmetric_matrix() {
        // unit gamma: upper[f] == lower[f] and both are -coeff
        let m = unit_mesh();
        let gamma = uniform_gamma(m.clone(), 1.0);
        let phi = VolScalarField::uniform("T", m.clone(), 0.0);
        let mat = laplacian(&gamma, &phi);
        // internal face: |C_N - C_O| = 0.5, area = 1 → coeff = 1/0.5 = 2
        assert!((mat.ldu.upper[0] - (-2.0)).abs() < 1e-10);
        assert!((mat.ldu.lower[0] - (-2.0)).abs() < 1e-10);
    }

    #[test]
    fn laplacian_solves_linear_dirichlet() {
        // −∇²T = 0, T(0)=0, T(1)=1 → T is linear: T[0]=0.25, T[1]=0.75
        let m = unit_mesh();
        let gamma = uniform_gamma(m.clone(), 1.0);
        let t_bc = vec![
            PatchField {
                bc: BoundaryCondition::FixedValue(1.0),
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::FixedValue(0.0),
                values: Field::new(vec![0.0]),
            },
        ];
        let phi = VolScalarField::new("T", m.clone(), Field::zeros(2), t_bc);
        let mat = laplacian(&gamma, &phi);
        let settings = SolverSettings::default();
        let (result, perf) = mat.solve("T", settings);
        assert!(perf.converged, "Gauss-Seidel did not converge");
        assert!(
            (result.internal[0] - 0.25).abs() < 1e-6,
            "T[0] = {}",
            result.internal[0]
        );
        assert!(
            (result.internal[1] - 0.75).abs() < 1e-6,
            "T[1] = {}",
            result.internal[1]
        );
    }
}
