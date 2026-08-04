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

/// Implicit Gauss-orthogonal Laplacian: assembles the matrix for `−∇·(Γ∇φ)`.
///
/// ## Sign convention (matches OpenFOAM)
///
/// The returned matrix has **positive** diagonal and **negative** off-diagonals,
/// so the matrix–vector product `A·φ` approximates `−∇·(Γ∇φ)`.  Use the matrix
/// with a minus sign in the PDE to add the diffusion term:
///
/// ```text
/// // ∂φ/∂t − ∇·(Γ∇φ) = S
/// let eqn = fvm::ddt(&phi, &phi_old, dt) - fvm::laplacian(&gamma_f, &phi);
/// ```
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
                // No-slip wall: fixedValue of zero → implicit diagonal only.
                BoundaryCondition::NoSlip => {
                    mat.ldu.diag[owner] += coeff;
                }
                // Prescribed normal gradient g [value·m⁻¹]: explicit boundary
                // flux gamma·area·g = (coeff·d)·g into the source, no diagonal.
                BoundaryCondition::FixedGradient(g) => {
                    mat.source[owner] += coeff * d * g;
                }
                // Robin/mixed: blend fixedValue (weight w) and fixedGradient
                // (1-w). w=1 recovers the FixedValue arm, w=0 the FixedGradient.
                BoundaryCondition::Mixed {
                    value_fraction,
                    ref_value,
                    ref_grad,
                } => {
                    let w = *value_fraction;
                    mat.ldu.diag[owner] += w * coeff;
                    mat.source[owner] += w * coeff * ref_value + (1.0 - w) * coeff * d * ref_grad;
                }
                // `InletOutlet`/`OutletInlet` carry no flux in the diffusion
                // operator (degrade to zero-gradient); `Slip`/`Wedge`/`Empty`
                // are zero-gradient-like here. No contribution.
                _ => {}
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

    fn uniform_gamma(m: Arc<crate::mesh::fv_mesh::FvMesh>, val: f64) -> SurfaceScalarField {
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
        let settings = crate::ldu_matrix::fv_matrix::SolverSettings::default();
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

    // ── V&V for the new patch-field BCs (verification, not validation) ──────
    // UNTRUSTED AI-ASSISTED DRAFT pending human V&V review.
    // Data version / date taken: 2026-08-04.

    /// V&V (verification, 2026-08-04). Methodology: assemble the Laplacian on
    /// the 2-cell unit mesh with a right patch that is (a) `FixedValue(v)` and
    /// (b) `Mixed { value_fraction: 1, ref_value: v, ref_grad: <arbitrary> }`.
    /// A mixed BC at `value_fraction == 1` must be identical to `fixedValue`.
    /// Pass criterion: every diag/source entry agrees to < 1e-12.
    /// Result: max |Δdiag| = 0, max |Δsource| = 0 (exact). PASS.
    #[test]
    fn vv_mixed_reduces_to_fixed_value() {
        let m = unit_mesh();
        let gamma = uniform_gamma(m.clone(), 1.0);
        let bc_fv = vec![
            PatchField {
                bc: BoundaryCondition::FixedValue(2.0),
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![0.0]),
            },
        ];
        let bc_mix = vec![
            PatchField {
                bc: BoundaryCondition::Mixed {
                    value_fraction: 1.0,
                    ref_value: 2.0,
                    ref_grad: 99.0, // must be ignored at w = 1
                },
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![0.0]),
            },
        ];
        let a = laplacian(&gamma, &VolScalarField::new("T", m.clone(), Field::zeros(2), bc_fv));
        let b = laplacian(&gamma, &VolScalarField::new("T", m.clone(), Field::zeros(2), bc_mix));
        for i in 0..2 {
            assert!((a.ldu.diag[i] - b.ldu.diag[i]).abs() < 1e-12);
            assert!((a.source[i] - b.source[i]).abs() < 1e-12);
        }
    }

    /// V&V (verification, 2026-08-04). Methodology: as above but comparing a
    /// right patch that is (a) `FixedGradient(g)` and (b)
    /// `Mixed { value_fraction: 0, ref_value: <arbitrary>, ref_grad: g }`.
    /// A mixed BC at `value_fraction == 0` must be identical to `fixedGradient`.
    /// Pass criterion: every diag/source entry agrees to < 1e-12.
    /// Result: max |Δdiag| = 0, max |Δsource| = 0 (exact). PASS.
    #[test]
    fn vv_mixed_reduces_to_fixed_gradient() {
        let m = unit_mesh();
        let gamma = uniform_gamma(m.clone(), 1.0);
        let bc_fg = vec![
            PatchField {
                bc: BoundaryCondition::FixedGradient(0.5),
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![0.0]),
            },
        ];
        let bc_mix = vec![
            PatchField {
                bc: BoundaryCondition::Mixed {
                    value_fraction: 0.0,
                    ref_value: 99.0, // must be ignored at w = 0
                    ref_grad: 0.5,
                },
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![0.0]),
            },
        ];
        let a = laplacian(&gamma, &VolScalarField::new("T", m.clone(), Field::zeros(2), bc_fg));
        let b = laplacian(&gamma, &VolScalarField::new("T", m.clone(), Field::zeros(2), bc_mix));
        for i in 0..2 {
            assert!((a.ldu.diag[i] - b.ldu.diag[i]).abs() < 1e-12);
            assert!((a.source[i] - b.source[i]).abs() < 1e-12);
        }
    }

    /// V&V (verification, 2026-08-04). Methodology: 1-D steady conduction
    /// `-∇²T = 0` on the 2-cell unit mesh `x ∈ [0, 1]` (cell centres 0.25,
    /// 0.75). Left patch `FixedValue(0)` at x=0, right patch `FixedGradient(g)`
    /// at x=1 with g = 2.0 (outward-normal gradient). Analytic solution is
    /// linear with slope g: T(x) = g·x, i.e. T(0.25)=0.5, T(0.75)=1.5.
    /// Pass criterion: cell values within 1e-6 of analytic.
    /// Result: T[0] = 0.500000, T[1] = 1.500000 (matches analytic exactly to
    /// solver tolerance). PASS — a prescribed gradient reproduces the linear
    /// profile.
    #[test]
    fn vv_fixed_gradient_linear_profile() {
        let m = unit_mesh();
        let gamma = uniform_gamma(m.clone(), 1.0);
        let g = 2.0;
        let t_bc = vec![
            PatchField {
                bc: BoundaryCondition::FixedGradient(g), // right, x = 1
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::FixedValue(0.0), // left, x = 0
                values: Field::new(vec![0.0]),
            },
        ];
        let phi = VolScalarField::new("T", m.clone(), Field::zeros(2), t_bc);
        let mat = laplacian(&gamma, &phi);
        let settings = crate::ldu_matrix::fv_matrix::SolverSettings::default();
        let (result, perf) = mat.solve("T", settings);
        assert!(perf.converged);
        assert!((result.internal[0] - 0.5).abs() < 1e-6, "T[0]={}", result.internal[0]);
        assert!((result.internal[1] - 1.5).abs() < 1e-6, "T[1]={}", result.internal[1]);
    }

    /// V&V (verification, 2026-08-04). Robin / convective-boundary analytic
    /// case. Methodology: 1-D steady conduction `-k∇²T = 0`, k=1, on the 2-cell
    /// unit mesh `x ∈ [0,1]`. Left patch `FixedValue(T0)` with T0=100 at x=0.
    /// Right patch at x=1 is a convective (Robin) boundary
    /// `-k dT/dx = h (T - T_inf)` with h=1, T_inf=0, cast to the mixed form
    /// (ref_grad=0, ref_value=T_inf) with
    /// `value_fraction = alpha·d / (alpha·d + 1)`, alpha = h/k = 1, d = 0.25 →
    /// value_fraction = 0.2. Analytic solution: slope s = h(T_inf−T0)/(k+hL) =
    /// −50, T(x) = 100 − 50x, giving T(0.25)=87.5, T(0.75)=62.5.
    /// Pass criterion: cell values within 1e-4 of analytic.
    /// Result: T[0] = 87.5000, T[1] = 62.5000 (matches analytic). PASS — the
    /// Mixed BC reproduces the analytic Robin profile.
    #[test]
    fn vv_robin_convective_analytic() {
        let m = unit_mesh();
        let gamma = uniform_gamma(m.clone(), 1.0);
        // alpha = h/k = 1, d = 0.25 → w = alpha*d/(alpha*d + 1) = 0.2
        let w = 0.2;
        let t_bc = vec![
            PatchField {
                bc: BoundaryCondition::Mixed {
                    value_fraction: w,
                    ref_value: 0.0, // T_inf
                    ref_grad: 0.0,
                },
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::FixedValue(100.0), // left, x = 0
                values: Field::new(vec![0.0]),
            },
        ];
        let phi = VolScalarField::new("T", m.clone(), Field::zeros(2), t_bc);
        let mat = laplacian(&gamma, &phi);
        let settings = crate::ldu_matrix::fv_matrix::SolverSettings::default();
        let (result, perf) = mat.solve("T", settings);
        assert!(perf.converged);
        assert!((result.internal[0] - 87.5).abs() < 1e-4, "T[0]={}", result.internal[0]);
        assert!((result.internal[1] - 62.5).abs() < 1e-4, "T[1]={}", result.internal[1]);
    }
}
