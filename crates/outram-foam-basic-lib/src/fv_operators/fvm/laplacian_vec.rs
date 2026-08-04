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
use crate::fields::vol_field::{VolScalarField, VolVectorField};
use crate::fv_operators::fvc::interpolate;
use crate::ldu_matrix::fv_vector_matrix::FvVectorMatrix;
use crate::mesh::fv_mesh::{FvMesh, PatchKind};

/// Implicit vector Laplacian: `−∇·(γ ∇U)`.
///
/// Assembles the same LDU coefficients as the scalar `fvm::laplacian`,
/// but with a vector source (for Dirichlet boundary conditions on U).
///
/// OpenFOAM convention (positive sign convention):
/// `laplacian_vec(gamma, U)` represents `∇·(γ ∇U)` (note: positive,
/// matching the momentum diffusion term; subtract when assembling
/// `fvm::ddt(U) - fvm::laplacian(nu, U)`).
pub fn laplacian_vec(
    gamma: &VolScalarField,
    u: &VolVectorField,
    mesh: Arc<FvMesh>,
) -> FvVectorMatrix {
    let gamma_f = interpolate(gamma);
    let mut mat = FvVectorMatrix::new(mesh.clone());

    // Internal faces
    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let nb = mesh.neighbour[f];
        let sf = mesh.face_area_vectors[f];
        let delta = (mesh.cell_centres[nb] - mesh.cell_centres[o]).mag();
        let area = sf.mag();
        let coeff = gamma_f.internal[f] * area / delta;

        mat.ldu.diag[o] += coeff;
        mat.ldu.diag[nb] += coeff;
        mat.ldu.upper[f] -= coeff;
        mat.ldu.lower[f] -= coeff;
    }

    // Boundary faces
    for (pi, patch) in mesh.patches.iter().enumerate() {
        // Cyclic / cyclicAMI patches are handled as internal-like seam couplings
        // below.
        if patch.kind == PatchKind::Cyclic || patch.kind == PatchKind::CyclicAmi {
            continue;
        }
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            let owner = mesh.owner[gf];
            let sf = mesh.face_area_vectors[gf];
            let area = sf.mag();
            let cf = mesh.face_centres[gf];
            let delta = (cf - mesh.cell_centres[owner]).mag().max(1e-30);
            let coeff = gamma_f.boundary[pi].values[fi] * area / delta;

            match &u.boundary[pi].bc {
                // Dirichlet-type: wall value from boundary.values[fi].
                // `FlowRateInletVelocity` is a fixed inlet velocity the solver
                // hook wrote into `values`, so it assembles identically.
                BoundaryCondition::FixedValue(_)
                | BoundaryCondition::FixedField(_)
                | BoundaryCondition::Calculated(_)
                | BoundaryCondition::FlowRateInletVelocity { .. } => {
                    let u_wall = u.boundary[pi].values[fi];
                    mat.ldu.diag[owner] += coeff;
                    mat.source[owner] = mat.source[owner] + u_wall * coeff;
                }
                // No-slip wall: fixedValue of zero — implicit diagonal only.
                BoundaryCondition::NoSlip => {
                    mat.ldu.diag[owner] += coeff;
                }
                // Prescribed normal gradient g [T·m⁻¹]: explicit boundary flux
                // gamma·area·g = (coeff·delta)·g into the source.
                BoundaryCondition::FixedGradient(g) => {
                    mat.source[owner] = mat.source[owner] + *g * (coeff * delta);
                }
                // Robin/mixed: blend fixedValue (weight w) and fixedGradient (1-w).
                BoundaryCondition::Mixed {
                    value_fraction,
                    ref_value,
                    ref_grad,
                } => {
                    let w = *value_fraction;
                    mat.ldu.diag[owner] += w * coeff;
                    mat.source[owner] = mat.source[owner]
                        + *ref_value * (w * coeff)
                        + *ref_grad * ((1.0 - w) * coeff * delta);
                }
                // Zero-gradient-like (no implicit/explicit diffusion contribution).
                // `InletOutlet`/`OutletInlet`/`Freestream`/
                // `PressureInletOutletVelocity` carry no flux in the diffusion
                // operator, so they degrade to zero-gradient here; `Slip`/`Wedge`
                // are treated as zero-gradient at this Layer (see enum docs).
                // `FixedFluxPressure`/`TotalPressure` are scalar pressure BCs and
                // are not meaningful on a vector field — zero-gradient stand-in.
                BoundaryCondition::ZeroGradient
                | BoundaryCondition::Symmetry
                | BoundaryCondition::Slip
                | BoundaryCondition::Wedge
                | BoundaryCondition::InletOutlet { .. }
                | BoundaryCondition::OutletInlet { .. }
                | BoundaryCondition::Freestream { .. }
                | BoundaryCondition::PressureInletOutletVelocity
                | BoundaryCondition::FixedFluxPressure { .. }
                | BoundaryCondition::TotalPressure { .. }
                | BoundaryCondition::Empty => {}
            }
        }
    }

    // Cyclic (periodic) seam couplings: internal-face orthogonal diffusion
    // across the seam (mirrors the scalar `fvm::laplacian` cyclic loop).
    for (i, cc) in mesh.cyclic_couplings.iter().enumerate() {
        let cf = mesh.cyclic_coupling_face(i);
        let o = cc.owner;
        let nb = cc.neighbour;
        let d_a = (mesh.face_centres[cc.face_a] - mesh.cell_centres[o]).mag();
        let d_b = (mesh.face_centres[cc.face_b] - mesh.cell_centres[nb]).mag();
        let delta = (d_a + d_b).max(1e-30);
        let area = mesh.face_area_vectors[cc.face_a].mag();
        // Seam Γ from the half0 boundary (interpolate() fills cyclic patches with
        // the across-seam interpolated value).
        let gamma_seam = gamma_f.boundary[cc.patch_a].values[cc.local];
        let coeff = gamma_seam * area / delta;
        mat.ldu.diag[o] += coeff;
        mat.ldu.diag[nb] += coeff;
        mat.ldu.upper[cf] -= coeff;
        mat.ldu.lower[cf] -= coeff;
    }

    // Non-conformal (cyclicAMI) seam couplings: partial internal faces, one per
    // weighted (target, source) overlap (mirrors the scalar `fvm::laplacian`
    // AMI loop). Reduces to the plain cyclic seam in the matching limit.
    let mut acf = mesh.ami_ldu_start();
    for coupling in &mesh.ami_couplings {
        let o = coupling.target_cell;
        let d_t = (mesh.face_centres[coupling.target_face] - mesh.cell_centres[o]).mag();
        let gamma_seam = gamma_f.boundary[coupling.target_patch].values[coupling.local];
        for w in &coupling.weights {
            let nb = w.source_cell;
            let d_s = (mesh.face_centres[w.source_face] - mesh.cell_centres[nb]).mag();
            let delta = (d_t + d_s).max(1e-30);
            let coeff = gamma_seam * w.overlap_area / delta;
            mat.ldu.diag[o] += coeff;
            mat.ldu.diag[nb] += coeff;
            mat.ldu.upper[acf] -= coeff;
            mat.ldu.lower[acf] -= coeff;
            acf += 1;
        }
    }

    let _ = u; // consumed for BCs above; suppress dead-code lint
    mat
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
    use crate::fields::field::Field;
    use crate::ldu_matrix::fv_matrix::SolverSettings;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::primitives::Vector3;
    use approx::assert_relative_eq;

    fn line_mesh(n: usize) -> Arc<FvMesh> {
        let dx = 1.0 / n as f64;
        let n_int = n - 1;
        let mut owner = (0..n_int).collect::<Vec<_>>();
        let neighbour = (1..n).collect::<Vec<_>>();
        owner.push(n - 1); // right boundary
        owner.push(0); // left boundary
                       // neighbour list is only for internal faces
        let cell_volumes = vec![dx; n];
        let cell_centres = (0..n)
            .map(|i| Vector3::new((i as f64 + 0.5) * dx, 0.0, 0.0))
            .collect();
        let face_centres: Vec<_> = (0..n_int)
            .map(|f| Vector3::new((f as f64 + 1.0) * dx, 0.0, 0.0))
            .chain([Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.0)])
            .collect();
        let face_area_vectors: Vec<_> = (0..n_int)
            .map(|_| Vector3::new(1.0, 0.0, 0.0))
            .chain([Vector3::new(1.0, 0.0, 0.0), Vector3::new(-1.0, 0.0, 0.0)])
            .collect();
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(n)
                .n_internal_faces(n_int)
                .owner(owner)
                .neighbour(neighbour)
                .patches(vec![
                    BoundaryPatch::new("right", n_int, 1, PatchKind::Wall),
                    BoundaryPatch::new("left", n_int + 1, 1, PatchKind::Wall),
                ])
                .cell_volumes(cell_volumes)
                .cell_centres(cell_centres)
                .face_area_vectors(face_area_vectors)
                .face_centres(face_centres)
                .build()
                .unwrap(),
        )
    }

    /// V&V (verification, 2026-08-04). Cyclic (periodic) vector Laplacian.
    /// Methodology: on `periodic_1d(4, 1.0, 1.0)` with Γ = 1, assemble the vector
    /// Laplacian. Its scalar LDU (shared by all 3 components) must (i) hold the
    /// circulant periodic stencil — a cyclic seam coupling cell 0 ↔ cell 3 with
    /// off-diagonal −Γ·A/h = −4 and diag 2·4 = 8; and (ii) be conservative — the
    /// Laplacian of a uniform field is zero, `A·[1,1,1,1] = 0`. It also checks
    /// that adding an implicit time-derivative matrix (`fvm::ddt_vec`, which
    /// touches no seam) leaves the cyclic off-diagonal intact — i.e. the matrix
    /// `+` operator lines up because every matrix on this mesh shares the LDU
    /// face structure. Pass criteria: off-diag = −4, max |A·1| < 1e-12, sum
    /// preserved after `+`.
    /// Result (measured 2026-08-04): seam upper = −4.000000, diag = 8.000000,
    /// max |A·1| = 0.0, seam upper after ddt_vec + laplacian_vec = −4.000000.
    /// PASS.
    #[test]
    fn vv_cyclic_laplacian_vec_conserves() {
        use crate::fv_operators::fvm::ddt_vec;
        let m = Arc::new(crate::mesh::fv_mesh::FvMesh::periodic_1d(4, 1.0, 1.0));
        let gamma = VolScalarField::uniform("nu", m.clone(), 1.0);
        let u = VolVectorField::uniform("U", m.clone(), Vector3::new(1.0, 2.0, 3.0));
        let mat = laplacian_vec(&gamma, &u, m.clone());
        // Seam coupling is the last LDU face (after the 3 internal faces).
        let cf = m.cyclic_coupling_face(0);
        assert_relative_eq!(mat.ldu.upper[cf], -4.0, epsilon = 1e-12);
        assert_relative_eq!(mat.ldu.lower[cf], -4.0, epsilon = 1e-12);
        for c in 0..4 {
            assert_relative_eq!(mat.ldu.diag[c], 8.0, epsilon = 1e-12);
        }
        // Conservation: Laplacian of a uniform field is zero.
        let ones = vec![1.0; 4];
        for &y in mat.ldu.multiply(&ones).iter() {
            assert!(y.abs() < 1e-12, "A·1 = {y}");
        }
        // Matrix `+` lines up across operators (ddt_vec adds no seam term).
        let ddt = ddt_vec(&u, &u, 1.0, m.clone());
        let combined = ddt + laplacian_vec(&gamma, &u, m.clone());
        assert_relative_eq!(combined.ldu.upper[cf], -4.0, epsilon = 1e-12);
    }

    /// V&V (verification, 2026-08-04). **Non-conformal (2:1) AMI vector Laplacian
    /// is conservative** — the vector diffusion operator annihilates a uniform
    /// vector field across the non-conformal periodic seam.
    ///
    /// Methodology: `FvMesh::periodic_ring_ami(2, 4, 1.0, 1.0, 1.0)` (2:1 mid
    /// seam), Γ = 1. The scalar LDU (shared by all 3 components) must give
    /// `A·1 = 0` (uniform field unchanged), mirroring the scalar
    /// `fvm::laplacian` AMI conservation check but through the vector-matrix
    /// AMI-seam assembly. Pass criterion: `‖A·1‖∞ < 1e-12`.
    /// Result (measured 2026-08-04): max |A·1| = 0.0 (exact). PASS.
    #[test]
    fn vv_ami_nonconformal_laplacian_vec_conserves() {
        let ring = Arc::new(crate::mesh::fv_mesh::FvMesh::periodic_ring_ami(2, 4, 1.0, 1.0, 1.0));
        let gamma = VolScalarField::uniform("nu", ring.clone(), 1.0);
        let u = VolVectorField::uniform("U", ring.clone(), Vector3::new(1.0, 2.0, 3.0));
        let mat = laplacian_vec(&gamma, &u, ring.clone());
        let ones = vec![1.0; ring.n_cells];
        for (c, &y) in mat.ldu.multiply(&ones).iter().enumerate() {
            assert!(y.abs() < 1e-12, "A·1 not conserved at cell {c}: {y}");
        }
    }

    #[test]
    fn laplacian_vec_1d_linear_profile() {
        // Solve ∇²U = 0 with U=0 at x=0, U=(1,0,0) at x=1 → linear solution
        let n = 4;
        let m = line_mesh(n);
        let gamma = VolScalarField::uniform("nu", m.clone(), 1.0);

        // Build BCs: right (x=1): U=(1,0,0), left (x=0): U=(0,0,0)
        let u_bc = vec![
            PatchField {
                bc: BoundaryCondition::FixedValue(Vector3::new(1.0, 0.0, 0.0)),
                values: Field::new(vec![Vector3::new(1.0, 0.0, 0.0)]),
            },
            PatchField {
                bc: BoundaryCondition::FixedValue(Vector3::ZERO),
                values: Field::new(vec![Vector3::ZERO]),
            },
        ];
        let u_init = VolVectorField::new("U", m.clone(), Field::new(vec![Vector3::ZERO; n]), u_bc);
        let mat = laplacian_vec(&gamma, &u_init, m.clone());
        let (u, perf) = mat.solve("U", SolverSettings::default());
        assert!(perf.converged, "residual = {}", perf.final_residual);
        for c in 0..n {
            let x = (c as f64 + 0.5) / n as f64;
            assert_relative_eq!(u.internal[c].x, x, epsilon = 1e-2);
        }
    }
}
