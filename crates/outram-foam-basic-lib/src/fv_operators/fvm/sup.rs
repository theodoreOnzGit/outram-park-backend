// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
//   src/finiteVolume/finiteVolume/fvm/fvmSup.C
//   @ commit 481094fd (OpenFOAM v2606)
// Copyright (C) 2011-2016 OpenFOAM Foundation
// Copyright (C) 2022 OpenCFD Ltd.
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

//! Implicit / explicit **source** terms `Sp`, `Su`, `SuSp` (`fvmSup`).
//!
//! ## Sign convention (read before combining matrices)
//!
//! These operators reproduce OpenFOAM's `fvm::Sp/Su/SuSp` assembly exactly.
//! Both represent a term on the **left-hand side** of the discretised equation
//! with its natural positive sign:
//!
//! - `Sp(sp, φ)`  — *implicit* term `sp·φ`  ⇒  `diag[c] += V[c]·sp[c]`.
//! - `Su(su, φ)`  — *explicit* term `su`     ⇒  `source[c] −= V[c]·su[c]`.
//! - `SuSp(c, φ)` — split: the positive part of `c` goes implicit (diagonal),
//!   the negative part goes explicit (source), for diagonal dominance.
//!
//! `Su` subtracts into `source` because, in OpenFOAM, an explicit LHS term moves
//! to the right-hand side when the system is solved.  In this crate the OpenFOAM
//! idiom `LHS == RHS` is written as `LHS - RHS` using the `FvMatrix` /
//! `FvVectorMatrix` `Sub` operator (the same "subtract when assembling" note as
//! `fvm::laplacian`).  So an explicit source `S` you want on the RHS is added as
//! `... - fvm::su(S, φ)`; a stabilising sink `k·φ` on the LHS is `... + fvm::sp(k, φ)`.

use crate::fields::vol_field::{VolScalarField, VolVectorField};
use crate::ldu_matrix::fv_matrix::FvMatrix;
use crate::ldu_matrix::fv_vector_matrix::FvVectorMatrix;

// ── Scalar field ───────────────────────────────────────────────────────────────

/// Implicit source `Sp(sp, φ)` for a scalar field: `diag[c] += V[c]·sp[c]`.
///
/// Represents the implicit term `sp·φ` (rank preserved). `sp` is a per-cell
/// scalar coefficient field. Field ranks: `VolScalarField × VolScalarField → FvMatrix`.
pub fn sp(sp: &VolScalarField, phi: &VolScalarField) -> FvMatrix {
    let mesh = phi.mesh.clone();
    let mut mat = FvMatrix::new(mesh.clone());
    for c in 0..mesh.n_cells {
        mat.ldu.diag[c] += mesh.cell_volumes[c] * sp.internal[c];
    }
    mat
}

/// Explicit source `Su(su, φ)` for a scalar field: `source[c] −= V[c]·su[c]`.
///
/// Represents the explicit term `su` (a known per-cell field). Field ranks:
/// `VolScalarField × VolScalarField → FvMatrix`. See the module-level sign note.
pub fn su(su: &VolScalarField, phi: &VolScalarField) -> FvMatrix {
    let mesh = phi.mesh.clone();
    let mut mat = FvMatrix::new(mesh.clone());
    for c in 0..mesh.n_cells {
        mat.source[c] -= mesh.cell_volumes[c] * su.internal[c];
    }
    mat
}

/// Split source `SuSp(susp, φ)` for a scalar field.
///
/// Positive coefficient → implicit (`diag[c] += V·max(susp,0)`); negative
/// coefficient → explicit (`source[c] −= V·min(susp,0)·φ`). Keeps the diagonal
/// dominant regardless of the coefficient sign. Field ranks:
/// `VolScalarField × VolScalarField → FvMatrix`.
pub fn su_sp(susp: &VolScalarField, phi: &VolScalarField) -> FvMatrix {
    let mesh = phi.mesh.clone();
    let mut mat = FvMatrix::new(mesh.clone());
    for c in 0..mesh.n_cells {
        let v = mesh.cell_volumes[c];
        let coeff = susp.internal[c];
        mat.ldu.diag[c] += v * coeff.max(0.0);
        mat.source[c] -= v * coeff.min(0.0) * phi.internal[c];
    }
    mat
}

// ── Vector field ───────────────────────────────────────────────────────────────

/// Implicit source `Sp(sp, U)` for a vector field: `diag[c] += V[c]·sp[c]`.
///
/// The implicit coefficient `sp` is a **scalar** field — it multiplies all three
/// velocity components equally (matching the scalar LDU coefficients of
/// `FvVectorMatrix`). Field ranks: `VolScalarField × VolVectorField → FvVectorMatrix`.
pub fn sp_vec(sp: &VolScalarField, u: &VolVectorField) -> FvVectorMatrix {
    let mesh = u.mesh.clone();
    let mut mat = FvVectorMatrix::new(mesh.clone());
    for c in 0..mesh.n_cells {
        mat.ldu.diag[c] += mesh.cell_volumes[c] * sp.internal[c];
    }
    mat
}

/// Explicit source `Su(su, U)` for a vector field: `source[c] −= V[c]·su[c]`.
///
/// `su` is a per-cell **vector** field. Field ranks:
/// `VolVectorField × VolVectorField → FvVectorMatrix`. See the module-level sign note.
pub fn su_vec(su: &VolVectorField, u: &VolVectorField) -> FvVectorMatrix {
    let mesh = u.mesh.clone();
    let mut mat = FvVectorMatrix::new(mesh.clone());
    for c in 0..mesh.n_cells {
        mat.source[c] -= su.internal[c] * mesh.cell_volumes[c];
    }
    mat
}

/// Split source `SuSp(susp, U)` for a vector field.
///
/// Scalar coefficient `susp`: positive part implicit (`diag`), negative part
/// explicit (`source`, applied to the vector `U`). Field ranks:
/// `VolScalarField × VolVectorField → FvVectorMatrix`.
pub fn su_sp_vec(susp: &VolScalarField, u: &VolVectorField) -> FvVectorMatrix {
    let mesh = u.mesh.clone();
    let mut mat = FvVectorMatrix::new(mesh.clone());
    for c in 0..mesh.n_cells {
        let v = mesh.cell_volumes[c];
        let coeff = susp.internal[c];
        mat.ldu.diag[c] += v * coeff.max(0.0);
        mat.source[c] -= u.internal[c] * (v * coeff.min(0.0));
    }
    mat
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ldu_matrix::fv_matrix::SolverSettings;
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

    /// V&V — methodology: `Sp(k)` puts `V·k` on the diagonal; `Su(S)` puts
    /// `−V·S` in the source. Assembling the OpenFOAM `Sp(k,φ) == Su(S,φ)` as
    /// `sp - su` must solve `k·φ = S`. With k=2, S=6, V=1 ⇒ φ = 3.
    ///
    /// Result (measured 2026-07-15): diag = 2, source(sp−su) = 6, solved φ = 3
    /// to < 1e-8, converged.
    #[test]
    fn sp_su_scalar_solve_k_phi_equals_s() {
        let m = unit_mesh();
        let phi = VolScalarField::zeros("phi", m.clone());
        let k = VolScalarField::uniform("k", m.clone(), 2.0);
        let s = VolScalarField::uniform("S", m.clone(), 6.0);

        let sp_m = sp(&k, &phi);
        assert!((sp_m.ldu.diag[0] - 2.0).abs() < 1e-12);
        let su_m = su(&s, &phi);
        assert!((su_m.source[0] - (-6.0)).abs() < 1e-12);

        let eqn = sp_m - su_m; // OpenFOAM `Sp(k,phi) == Su(S,phi)`
        assert!((eqn.source[0] - 6.0).abs() < 1e-12);
        let (x, perf) = eqn.solve("phi", SolverSettings::default());
        assert!(perf.converged, "residual = {}", perf.final_residual);
        assert!(
            (x.internal[0] - 3.0).abs() < 1e-8,
            "phi = {}",
            x.internal[0]
        );
    }

    /// V&V — methodology: `SuSp` routes coefficient by sign. Positive coeff
    /// (+4) → diagonal, source untouched; negative coeff (−4) applied to φ=5
    /// → `source −= V·(−4)·5 = +20`, diagonal untouched.
    ///
    /// Result (measured 2026-07-15): positive → diag=4, source=0;
    /// negative → diag=0, source=20.
    #[test]
    fn su_sp_scalar_routes_by_sign() {
        let m = unit_mesh();
        let mut phi = VolScalarField::uniform("phi", m.clone(), 5.0);
        phi.internal[1] = 5.0;

        let pos = VolScalarField::uniform("cp", m.clone(), 4.0);
        let mp = su_sp(&pos, &phi);
        assert!((mp.ldu.diag[0] - 4.0).abs() < 1e-12);
        assert!(mp.source[0].abs() < 1e-12);

        let neg = VolScalarField::uniform("cn", m.clone(), -4.0);
        let mn = su_sp(&neg, &phi);
        assert!(mn.ldu.diag[0].abs() < 1e-12);
        assert!(
            (mn.source[0] - 20.0).abs() < 1e-12,
            "source = {}",
            mn.source[0]
        );
    }

    /// V&V — methodology: vector `Sp(k,U) == Su(S,U)` solves `k·U = S`
    /// component-wise. k=2, S=(6,−4,10), V=1 ⇒ U = (3,−2,5).
    ///
    /// Result (measured 2026-07-15): solved U = (3, −2, 5) to < 1e-8, converged.
    #[test]
    fn sp_su_vector_solve_componentwise() {
        let m = unit_mesh();
        let u = VolVectorField::zero("U", m.clone());
        let k = VolScalarField::uniform("k", m.clone(), 2.0);
        let s = VolVectorField::uniform("S", m.clone(), Vector3::new(6.0, -4.0, 10.0));

        let eqn = sp_vec(&k, &u) - su_vec(&s, &u);
        let (x, perf) = eqn.solve("U", SolverSettings::default());
        assert!(perf.converged, "residual = {}", perf.final_residual);
        assert!(
            (x.internal[0].x - 3.0).abs() < 1e-8,
            "x = {}",
            x.internal[0].x
        );
        assert!(
            (x.internal[0].y - (-2.0)).abs() < 1e-8,
            "y = {}",
            x.internal[0].y
        );
        assert!(
            (x.internal[0].z - 5.0).abs() < 1e-8,
            "z = {}",
            x.internal[0].z
        );
    }

    /// V&V — methodology: vector `SuSp` with a negative coefficient (−3) applied
    /// to U=(2,0,0), V=1 ⇒ `source −= V·(−3)·U = +(6,0,0)`, diagonal untouched.
    ///
    /// Result (measured 2026-07-15): diag=0, source=(6,0,0).
    #[test]
    fn su_sp_vector_negative_goes_explicit() {
        let m = unit_mesh();
        let u = VolVectorField::uniform("U", m.clone(), Vector3::new(2.0, 0.0, 0.0));
        let neg = VolScalarField::uniform("cn", m.clone(), -3.0);
        let mn = su_sp_vec(&neg, &u);
        assert!(mn.ldu.diag[0].abs() < 1e-12);
        assert!(
            (mn.source[0].x - 6.0).abs() < 1e-12,
            "source.x = {}",
            mn.source[0].x
        );
    }
}
