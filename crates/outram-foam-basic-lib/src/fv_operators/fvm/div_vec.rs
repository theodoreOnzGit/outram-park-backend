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
use crate::ldu_matrix::fv_vector_matrix::FvVectorMatrix;
use crate::mesh::fv_mesh::{FvMesh, PatchKind};

/// Implicit upwind convection of a vector field `U` by a face flux `phi`:
/// `∇·(φ U)` (assembles into a `FvVectorMatrix`).
///
/// Sign convention:
/// - `phi[f] ≥ 0` → flux flows from owner to neighbour (upwind = owner):
///   - `diag[owner] += phi[f]`  (implicit term on `U[owner]`)
///   - `diag[nbr]   -= 0`       (no contribution)
///   - `upper[f]    += phi[f].min(0.0) = 0`
/// - `phi[f] < 0`  → flux flows from neighbour to owner (upwind = neighbour):
///   - `upper[f]    += phi[f]`  (implicit off-diagonal)
///   - `diag[nbr]   -= phi[f]`
///
/// Mirrors `fvm::div(phi, U)` with the upwind convection scheme.
pub fn div_vec(phi: &SurfaceScalarField, u: &VolVectorField, mesh: Arc<FvMesh>) -> FvVectorMatrix {
    let mut mat = FvVectorMatrix::new(mesh.clone());

    // Internal faces
    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let nb = mesh.neighbour[f];
        let phi_f = phi.internal[f];

        mat.ldu.diag[o] += phi_f.max(0.0);
        mat.ldu.upper[f] += phi_f.min(0.0);
        mat.ldu.diag[nb] -= phi_f.min(0.0);
        mat.ldu.lower[f] -= phi_f.max(0.0);
    }

    // Boundary faces: explicit contribution (upwind = owner cell)
    for (pi, patch) in mesh.patches.iter().enumerate() {
        // Cyclic / cyclicAMI patches are handled as internal-like seam couplings
        // below.
        if patch.kind == PatchKind::Cyclic || patch.kind == PatchKind::CyclicAmi {
            continue;
        }
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
                // No-slip wall: fixedValue of zero → explicit source is zero;
                // the boundary face contributes nothing to owner's row.
                BoundaryCondition::NoSlip => {}
                // Flux-switched: inletOutlet imposes inletValue on inflow
                // (phi_f < 0) and is zeroGradient on outflow (phi_f ≥ 0).
                BoundaryCondition::InletOutlet { inlet_value } => {
                    if phi_f >= 0.0 {
                        mat.ldu.diag[owner] += phi_f; // outflow: zeroGradient (upwind owner)
                    } else {
                        mat.source[owner] = mat.source[owner] - inlet_value * phi_f;
                    }
                }
                BoundaryCondition::OutletInlet { outlet_value } => {
                    if phi_f >= 0.0 {
                        mat.source[owner] = mat.source[owner] - outlet_value * phi_f;
                    } else {
                        mat.ldu.diag[owner] += phi_f; // inflow: zeroGradient (upwind owner)
                    }
                }
                // freestream = inletOutlet(freestreamValue): fixedValue on
                // inflow (phi_f < 0), zeroGradient on outflow (phi_f ≥ 0).
                BoundaryCondition::Freestream { freestream_value } => {
                    if phi_f >= 0.0 {
                        mat.ldu.diag[owner] += phi_f; // outflow: zeroGradient
                    } else {
                        mat.source[owner] = mat.source[owner] - freestream_value * phi_f;
                    }
                }
                // pressureInletOutletVelocity: zeroGradient on outflow; on inflow
                // the fixedValue is the solver-computed normal velocity stored in
                // the patch (refreshed via update_pressure_inlet_outlet_velocity).
                BoundaryCondition::PressureInletOutletVelocity => {
                    if phi_f >= 0.0 {
                        mat.ldu.diag[owner] += phi_f; // outflow: zeroGradient
                    } else {
                        let u_bc = u.boundary[pi].values[fi];
                        mat.source[owner] = mat.source[owner] - u_bc * phi_f;
                    }
                }
                // flowRateInletVelocity: a fixed inlet velocity (Dirichlet) held
                // in the patch values — explicit source like FixedValue.
                BoundaryCondition::FlowRateInletVelocity { .. } => {
                    let u_bc = u.boundary[pi].values[fi];
                    mat.source[owner] = mat.source[owner] - u_bc * phi_f;
                }
                _ => {}
            }
        }
    }

    // Cyclic (periodic) seam couplings: first-order upwind across the seam
    // (mirrors the scalar `fvm::div` cyclic loop).
    for (i, cc) in mesh.cyclic_couplings.iter().enumerate() {
        let cf = mesh.cyclic_coupling_face(i);
        let o = cc.owner;
        let nb = cc.neighbour;
        let phi_f = phi.boundary[cc.patch_a].values[cc.local];
        mat.ldu.diag[o] += phi_f.max(0.0);
        mat.ldu.upper[cf] += phi_f.min(0.0);
        mat.ldu.diag[nb] -= phi_f.min(0.0);
        mat.ldu.lower[cf] -= phi_f.max(0.0);
    }

    // Non-conformal (cyclicAMI) seam couplings: first-order upwind of the
    // weight-split sub-flux `φ_target · weight` per (target, source) overlap
    // (mirrors the scalar `fvm::div` AMI loop).
    let mut acf = mesh.ami_ldu_start();
    for coupling in &mesh.ami_couplings {
        let o = coupling.target_cell;
        let phi_target = phi.boundary[coupling.target_patch].values[coupling.local];
        for w in &coupling.weights {
            let nb = w.source_cell;
            let phi_f = phi_target * w.weight;
            mat.ldu.diag[o] += phi_f.max(0.0);
            mat.ldu.upper[acf] += phi_f.min(0.0);
            mat.ldu.diag[nb] -= phi_f.min(0.0);
            mat.ldu.lower[acf] -= phi_f.max(0.0);
            acf += 1;
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
    fn zero_flux_gives_zero_matrix() {
        let m = two_cell_mesh();
        let u = VolVectorField::uniform("U", m.clone(), Vector3::new(1.0, 0.0, 0.0));
        let phi_bnd: Vec<_> = m
            .patches
            .iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::uniform(p.size, 0.0),
            })
            .collect();
        let phi = SurfaceScalarField::new("phi", m.clone(), Field::uniform(1, 0.0), phi_bnd);
        let mat = div_vec(&phi, &u, m);
        assert!(mat.ldu.diag.iter().all(|&d| d == 0.0));
        assert!(mat.ldu.upper.iter().all(|&d| d == 0.0));
    }

    /// V&V (verification, 2026-08-04). **Non-conformal (2:1) AMI vector
    /// convection is conservative** — a uniform field advects around the
    /// non-conformal periodic loop unchanged (`A·1 = 0`), through the
    /// vector-matrix AMI-seam assembly.
    ///
    /// Methodology: `FvMesh::periodic_ring_ami(2, 4, 1.0, 1.0, 1.0)`, uniform
    /// `U = (1,0,0)` face flux `φ = U·Sf` on every seam face, `fvm::div_vec`
    /// assembled. Pass criterion: `‖A·1‖∞ < 1e-12`.
    /// Result (measured 2026-08-04): max |A·1| = 0.0 (exact). PASS.
    #[test]
    fn vv_ami_nonconformal_div_vec_conserves() {
        let ring = Arc::new(crate::mesh::fv_mesh::FvMesh::periodic_ring_ami(
            2, 4, 1.0, 1.0, 1.0,
        ));
        let boundary = ring
            .patches
            .iter()
            .map(|p| {
                let vals: Vec<f64> = (0..p.size)
                    .map(|fi| ring.face_area_vectors[p.start + fi].x) // u = 1
                    .collect();
                PatchField {
                    bc: BoundaryCondition::ZeroGradient,
                    values: Field::new(vals),
                }
            })
            .collect();
        let phi = SurfaceScalarField::new("phi", ring.clone(), Field::uniform(0, 0.0), boundary);
        let u = VolVectorField::uniform("U", ring.clone(), Vector3::new(1.0, 2.0, 3.0));
        let mat = div_vec(&phi, &u, ring.clone());
        let ones = vec![1.0; ring.n_cells];
        for (c, &y) in mat.ldu.multiply(&ones).iter().enumerate() {
            assert!(y.abs() < 1e-12, "A·1 not conserved at cell {c}: {y}");
        }
    }
}
