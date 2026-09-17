// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
//   src/finiteVolume/finiteVolume/fvm/fvmD2dt2.C
//   src/finiteVolume/finiteVolume/d2dt2Schemes/EulerD2dt2Scheme/EulerD2dt2Scheme.C
//   @ commit 481094fd (OpenFOAM v2606)
// Copyright (C) 2011-2016 OpenFOAM Foundation
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
use crate::ldu_matrix::fv_vector_matrix::FvVectorMatrix;
use crate::mesh::fv_mesh::FvMesh;

/// Implicit second time derivative `∂²U/∂t²` for a `VolVectorField`,
/// Euler backward-difference (`EulerD2dt2Scheme`) → `FvVectorMatrix`.
///
/// # Math
///
/// The Euler `d2dt2` scheme uses the current value and the two previous states
/// (`U_old` at t−Δt, `U_oldold` at t−2Δt).  For a **constant timestep**
/// (`Δt = Δt₀`, which is what this Euler ddt layer supports) the OpenFOAM
/// coefficients collapse to `coefft = coefft00 = 1`, `coefft0 = 2`, and
/// `rΔt² = 4/(Δt+Δt₀)² = 1/Δt²`, so the volume-integrated term is
///
/// `∫_V ∂²U/∂t² dV ≈ (V/Δt²)·(U − 2·U_old + U_oldold)`
///
/// which is the standard central second difference.  This assembles into the
/// implicit system as:
///
/// - `diag[c] += V[c] / Δt²`
/// - `source[c] += (V[c] / Δt²)·(2·U_old[c] − U_oldold[c])`
///
/// With no spatial terms the solved field satisfies `U = 2·U_old − U_oldold`
/// (zero acceleration), i.e. the discrete `(U − 2U_old + U_oldold)/Δt² = 0`.
///
/// # Field ranks
///
/// input `VolVectorField` (current, old, old-old) → `FvVectorMatrix`
/// (scalar LDU coefficients, `Vector3` source).
///
/// # Assumptions
///
/// Constant timestep (`Δt = Δt₀`) and a non-moving mesh — matching the Euler
/// `ddt`/`ddt_vec` operators already in this crate.  For a variable timestep the
/// general `EulerD2dt2Scheme` coefficients would be needed; that is not modelled
/// here (the mesh has no `V0`/`V00` history).  This is the transient term of a
/// `solidDisplacementFoam`-style `DEqn` (`fvm::d2dt2(D) == ...`).
pub fn d2dt2(
    u: &VolVectorField,
    u_old: &VolVectorField,
    u_oldold: &VolVectorField,
    dt: f64,
    mesh: Arc<FvMesh>,
) -> FvVectorMatrix {
    let n = mesh.n_cells;
    let r_dt2 = 1.0 / (dt * dt);
    let mut mat = FvVectorMatrix::new(mesh.clone());
    for c in 0..n {
        let coeff = mesh.cell_volumes[c] * r_dt2;
        mat.ldu.diag[c] += coeff;
        // source += (V/Δt²)·(2·U_old − U_oldold)
        mat.source[c] += (u_old.internal[c] * 2.0 - u_oldold.internal[c]) * coeff;
    }
    let _ = u; // type ensures the correct VolVectorField is passed
    mat
}

/// Density-weighted implicit `∂²(ρ·U)/∂t²` for a `VolVectorField` → `FvVectorMatrix`.
///
/// As [`d2dt2`] but scaled by a scalar coefficient field `ρ` (constant in time
/// in this constant-timestep, non-moving-mesh form):
///
/// - `diag[c] += ρ[c]·V[c] / Δt²`
/// - `source[c] += ρ[c]·(V[c] / Δt²)·(2·U_old[c] − U_oldold[c])`
///
/// Covers `fvm::d2dt2(rho, U)` (e.g. inertial term `ρ ∂²D/∂t²` in an elastic
/// momentum balance).
pub fn d2dt2_coeff(
    rho: &VolScalarField,
    u: &VolVectorField,
    u_old: &VolVectorField,
    u_oldold: &VolVectorField,
    dt: f64,
    mesh: Arc<FvMesh>,
) -> FvVectorMatrix {
    let n = mesh.n_cells;
    let r_dt2 = 1.0 / (dt * dt);
    let mut mat = FvVectorMatrix::new(mesh.clone());
    for c in 0..n {
        let coeff = rho.internal[c] * mesh.cell_volumes[c] * r_dt2;
        mat.ldu.diag[c] += coeff;
        mat.source[c] += (u_old.internal[c] * 2.0 - u_oldold.internal[c]) * coeff;
    }
    let _ = u;
    mat
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ldu_matrix::fv_matrix::SolverSettings;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::primitives::Vector3;

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

    /// V&V — methodology: sample a **quadratic-in-time** signal
    /// `U(t) = a + b·t + c·t²` (per component; here a=1, b=(2,−1,3),
    /// c=(0.5,4,−2)) at t, t−Δt, t−2Δt with Δt=0.1.  The operator represents the
    /// volume-integrated `∂²U/∂t²`, so the per-unit-volume residual
    /// `(diag[c]·U[c] − source[c]) / V[c]` must equal the exact constant
    /// acceleration `2·c` for a quadratic — independent of t.
    ///
    /// Analytic: `U − 2U_old + U_oldold = 2c·Δt²`, divided by `V·Δt²/V·... `
    /// → `2c`.
    ///
    /// Result (measured 2026-07-15): residual/V = (1, 8, −4) = 2·c to < 1e-9 in
    /// both cells.
    #[test]
    fn d2dt2_of_quadratic_returns_constant_acceleration() {
        let m = unit_mesh();
        let dt = 0.1_f64;
        let t = 0.7_f64;
        let a = Vector3::new(1.0, 1.0, 1.0);
        let b = Vector3::new(2.0, -1.0, 3.0);
        let c = Vector3::new(0.5, 4.0, -2.0);
        let quad = |tt: f64| a + b * tt + c * (tt * tt);

        let u = VolVectorField::uniform("U", m.clone(), quad(t));
        let u_old = VolVectorField::uniform("Uo", m.clone(), quad(t - dt));
        let u_oldold = VolVectorField::uniform("Uoo", m.clone(), quad(t - 2.0 * dt));

        let mat = d2dt2(&u, &u_old, &u_oldold, dt, m.clone());
        let expected = c * 2.0; // 2c = (1, 8, -4)
        for cell in 0..2 {
            let resid = (u.internal[cell] * mat.ldu.diag[cell] - mat.source[cell])
                * (1.0 / m.cell_volumes[cell]);
            assert!(
                (resid.x - expected.x).abs() < 1e-9,
                "cell {cell} x = {}",
                resid.x
            );
            assert!(
                (resid.y - expected.y).abs() < 1e-9,
                "cell {cell} y = {}",
                resid.y
            );
            assert!(
                (resid.z - expected.z).abs() < 1e-9,
                "cell {cell} z = {}",
                resid.z
            );
        }
    }

    /// V&V — methodology: with only the `d2dt2` term (no spatial coupling), the
    /// implicit solve must return `U = 2·U_old − U_oldold` (zero acceleration
    /// continuation of a linear trajectory).
    ///
    /// U_old = (1,2,3), U_oldold = (0,1,2) ⇒ U = (2,3,4).
    ///
    /// Result (measured 2026-07-15): solved U = (2, 3, 4) to < 1e-7, converged.
    #[test]
    fn d2dt2_no_spatial_terms_continues_linearly() {
        let m = unit_mesh();
        let u = VolVectorField::zero("U", m.clone());
        let u_old = VolVectorField::uniform("Uo", m.clone(), Vector3::new(1.0, 2.0, 3.0));
        let u_oldold = VolVectorField::uniform("Uoo", m.clone(), Vector3::new(0.0, 1.0, 2.0));
        let mat = d2dt2(&u, &u_old, &u_oldold, 0.05, m.clone());
        let (u_new, perf) = mat.solve("U", SolverSettings::default());
        assert!(perf.converged, "residual = {}", perf.final_residual);
        assert!((u_new.internal[0].x - 2.0).abs() < 1e-7);
        assert!((u_new.internal[0].y - 3.0).abs() < 1e-7);
        assert!((u_new.internal[0].z - 4.0).abs() < 1e-7);
    }

    /// V&V — methodology: `d2dt2_coeff` scales the diagonal by ρ.
    /// ρ=3, V=1, Δt=0.5 ⇒ diag = 3·1/0.25 = 12; source = 12·(2·U_old − U_oldold).
    ///
    /// Result (measured 2026-07-15): diag = 12; source.x = 12·(2·5 − 1) = 108.
    #[test]
    fn d2dt2_coeff_scales_diagonal_by_rho() {
        let m = unit_mesh();
        let rho = VolScalarField::uniform("rho", m.clone(), 3.0);
        let u = VolVectorField::zero("U", m.clone());
        let u_old = VolVectorField::uniform("Uo", m.clone(), Vector3::new(5.0, 0.0, 0.0));
        let u_oldold = VolVectorField::uniform("Uoo", m.clone(), Vector3::new(1.0, 0.0, 0.0));
        let mat = d2dt2_coeff(&rho, &u, &u_old, &u_oldold, 0.5, m);
        assert!(
            (mat.ldu.diag[0] - 12.0).abs() < 1e-12,
            "diag = {}",
            mat.ldu.diag[0]
        );
        assert!(
            (mat.source[0].x - 108.0).abs() < 1e-12,
            "source.x = {}",
            mat.source[0].x
        );
    }
}
