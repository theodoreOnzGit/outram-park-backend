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
                // Flux-switched: inletOutlet = fixedValue(inletValue) on inflow
                // (phi_f < 0), zeroGradient on outflow (phi_f ≥ 0). outletInlet
                // is the mirror. The switch uses the sign of the outward face
                // flux phi_f = U·Sf, decided here where the flux is available.
                BoundaryCondition::InletOutlet { inlet_value } => {
                    if phi_f >= 0.0 {
                        mat.ldu.diag[owner] += phi_f; // outflow: zeroGradient
                    } else {
                        mat.source[owner] -= phi_f * inlet_value; // inflow: fixedValue
                    }
                }
                BoundaryCondition::OutletInlet { outlet_value } => {
                    if phi_f >= 0.0 {
                        mat.source[owner] -= phi_f * outlet_value; // outflow: fixedValue
                    } else {
                        mat.ldu.diag[owner] += phi_f; // inflow: zeroGradient
                    }
                }
                // Zero-gradient-like for convection (Slip/Wedge/NoSlip/Empty/
                // FixedGradient/Mixed): upwind donor is the owner cell on
                // outflow; inflow carries no known explicit value here.
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
    use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
    use crate::fields::field::Field;
    use crate::fields::surface_field::SurfaceScalarField;
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

    fn make_phi(
        m: Arc<crate::mesh::fv_mesh::FvMesh>,
        internal: f64,
        bnd: f64,
    ) -> SurfaceScalarField {
        let n_int = m.n_internal_faces;
        let bnd_vals: Vec<_> = m
            .patches
            .iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::uniform(p.size, bnd),
            })
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
        assert!(
            (mat.ldu.diag[0] - 1.0).abs() < 1e-12,
            "diag[0]={}",
            mat.ldu.diag[0]
        );
        assert!((mat.ldu.upper[0] - 0.0).abs() < 1e-12);
        assert!((mat.ldu.diag[1] - 0.0).abs() < 1e-12);
        assert!(
            (mat.ldu.lower[0] - (-1.0)).abs() < 1e-12,
            "lower[0]={}",
            mat.ldu.lower[0]
        );
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

    /// Build phi with a per-patch boundary flux (right patch = `right_phi`,
    /// left patch = 0) and zero internal flux, to isolate the right patch.
    fn phi_right_only(m: Arc<crate::mesh::fv_mesh::FvMesh>, right_phi: f64) -> SurfaceScalarField {
        let bnd = vec![
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![right_phi]), // patch 0 = "right"
            },
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![0.0]), // patch 1 = "left"
            },
        ];
        SurfaceScalarField::new("phi", m.clone(), Field::uniform(m.n_internal_faces, 0.0), bnd)
    }

    /// V&V (verification, 2026-08-04). inletOutlet flux switch. Methodology:
    /// isolate the right patch (owner = cell 1) of the 2-cell unit mesh, zero
    /// internal and left-patch flux, and set the scalar field's right patch to
    /// `InletOutlet { inlet_value: 5.0 }`. Assemble `fvm::div` twice:
    /// - outflow (phi_f = +2 ≥ 0): must act as zeroGradient → `diag[1] += 2`,
    ///   `source[1]` unchanged (0);
    /// - inflow  (phi_f = −2 < 0): must act as fixedValue → `source[1] -=
    ///   phi_f·inlet_value = 10`, `diag[1]` unchanged (0).
    /// Pass criterion: both assembled entries match to < 1e-12.
    /// Result: outflow diag[1] = 2.000000, source[1] = 0; inflow diag[1] = 0,
    /// source[1] = 10.000000. PASS — the switch flips with the flux sign.
    #[test]
    fn vv_inlet_outlet_flux_switch() {
        let m = unit_mesh();
        let make_psi = || {
            let bc = vec![
                PatchField {
                    bc: BoundaryCondition::InletOutlet { inlet_value: 5.0 },
                    values: Field::new(vec![0.0]),
                },
                PatchField {
                    bc: BoundaryCondition::ZeroGradient,
                    values: Field::new(vec![0.0]),
                },
            ];
            VolScalarField::new("psi", m.clone(), Field::zeros(2), bc)
        };

        // Outflow: zeroGradient behaviour.
        let out = div(&phi_right_only(m.clone(), 2.0), &make_psi());
        assert!((out.ldu.diag[1] - 2.0).abs() < 1e-12, "diag[1]={}", out.ldu.diag[1]);
        assert!(out.source[1].abs() < 1e-12, "source[1]={}", out.source[1]);

        // Inflow: fixedValue behaviour, source[1] -= phi_f * inlet_value = 10.
        let inn = div(&phi_right_only(m.clone(), -2.0), &make_psi());
        assert!(inn.ldu.diag[1].abs() < 1e-12, "diag[1]={}", inn.ldu.diag[1]);
        assert!((inn.source[1] - 10.0).abs() < 1e-12, "source[1]={}", inn.source[1]);
    }
}
