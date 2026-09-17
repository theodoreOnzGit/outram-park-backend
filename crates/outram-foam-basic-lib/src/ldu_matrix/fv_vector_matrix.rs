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

use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};
use std::sync::Arc;

use super::fv_matrix::{SolverPerformance, SolverSettings};
use super::ldu_matrix::LduMatrix;
use super::solvers::gauss_seidel::gauss_seidel as gs;
use super::solvers::krylov_solve::{krylov_solve, KrylovMethod, KrylovOptions};
use crate::fields::boundary::bc::PatchField;
use crate::fields::field::Field;
use crate::fields::vol_field::{VolScalarField, VolVectorField};
use crate::mesh::fv_mesh::FvMesh;
use crate::primitives::Vector3;

/// Implicit vector equation `A·U = b` for a `VolVectorField`.
///
/// Mirrors `Foam::fvVectorMatrix` (`fvMatrix<vector>`).
///
/// The LDU coefficients are **scalar** — they multiply the entire velocity
/// vector equally in all three directions.  The source vector is a
/// `Field<Vector3>`.  Solving decomposes into three independent scalar
/// Gauss-Seidel solves (one per component).
#[derive(Debug, Clone)]
pub struct FvVectorMatrix {
    /// Mesh the equation is defined on (shares the face addressing).
    pub mesh: Arc<FvMesh>,
    /// Scalar LDU coefficients of the operator `A` (shared by all 3 components).
    pub ldu: LduMatrix,
    /// Right-hand-side vector source per cell, length `n_cells`.
    pub source: Field<Vector3>,
}

impl FvVectorMatrix {
    /// Allocate a zero-initialised vector matrix for `mesh` (zero coefficients,
    /// zero source).
    ///
    /// As with [`FvMatrix::new`](crate::ldu_matrix::FvMatrix::new), the LDU face
    /// addressing holds the internal faces, then one slot per
    /// [`CyclicCoupling`](crate::mesh::CyclicCoupling), then one slot per
    /// [`AmiWeight`](crate::mesh::AmiWeight) of each
    /// [`AmiCoupling`](crate::mesh::AmiCoupling), so every vector matrix on a
    /// given mesh shares one structure and the `+`/`−` operators line up.
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let n_cells = mesh.n_cells;
        let mut owner = mesh.owner[..mesh.n_internal_faces].to_vec();
        let mut neighbour = mesh.neighbour.clone();
        for cc in &mesh.cyclic_couplings {
            owner.push(cc.owner);
            neighbour.push(cc.neighbour);
        }
        // Non-conformal (AMI) seams: one LDU face per weighted (target, source)
        // pair, appended after the cyclic couplings in `ami_couplings` order.
        for cc in &mesh.ami_couplings {
            for w in &cc.weights {
                owner.push(cc.target_cell);
                neighbour.push(w.source_cell);
            }
        }
        Self {
            ldu: LduMatrix::new(n_cells, owner, neighbour),
            source: Field::from_fn(n_cells, |_| Vector3::ZERO),
            mesh,
        }
    }

    /// Add `coeff[c]` to the diagonal of cell `c` (e.g. a time-derivative term).
    pub fn add_to_diag(&mut self, coeff: &Field<f64>) {
        for c in 0..self.mesh.n_cells {
            self.ldu.diag[c] += coeff[c];
        }
    }

    /// Add `term[c]` to the vector source of cell `c`.
    pub fn add_to_source(&mut self, term: &Field<Vector3>) {
        for c in 0..self.mesh.n_cells {
            self.source[c] = self.source[c] + term[c];
        }
    }

    /// Pin one cell's velocity to a fixed value (reference cell for closed domains).
    pub fn set_reference(&mut self, cell: usize, value: Vector3) {
        self.ldu.diag[cell] += 1e30;
        self.source[cell] = self.source[cell] + value * 1e30;
    }

    /// Diagonal coefficient per cell: `A[c] = diag[c]`.
    ///
    /// Used in PISO: `rAU = 1 / UEqn.a_field()`.
    pub fn a_field(&self) -> VolScalarField {
        let mesh = self.mesh.clone();
        let boundary = mesh
            .patches
            .iter()
            .map(|p| PatchField::zero_gradient(p.size))
            .collect();
        VolScalarField::new(
            "A",
            mesh.clone(),
            Field::new(self.ldu.diag.clone()),
            boundary,
        )
    }

    /// Off-diagonal + source residual: `H[c] = source[c] − Σ off-diag · U`.
    ///
    /// For a zero field x this returns `source[c]` directly.
    /// Used in PISO: `HbyA = rAU * UEqn.h_field(U)`.
    pub fn h_field(&self, u: &VolVectorField) -> VolVectorField {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;
        let mut h = vec![Vector3::ZERO; n];
        for c in 0..n {
            h[c] = self.source[c];
        }
        // Full LDU face count (internal + appended cyclic couplings) so
        // periodic-seam off-diagonals contribute to H.
        for f in 0..self.ldu.n_internal_faces {
            let o = self.ldu.owner[f];
            let nb = self.ldu.neighbour[f];
            h[o] = h[o] - u.internal[nb] * self.ldu.upper[f];
            h[nb] = h[nb] - u.internal[o] * self.ldu.lower[f];
        }
        let boundary = mesh
            .patches
            .iter()
            .map(|p| PatchField::zero_gradient_vec(p.size))
            .collect();
        VolVectorField::new("H", mesh, Field::new(h), boundary)
    }

    /// Solve each component (x, y, z) as an independent scalar Gauss-Seidel problem.
    pub fn solve(
        &self,
        name: &str,
        settings: SolverSettings,
    ) -> (VolVectorField, SolverPerformance) {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;

        let bx: Vec<f64> = (0..n).map(|c| self.source[c].x).collect();
        let by: Vec<f64> = (0..n).map(|c| self.source[c].y).collect();
        let bz: Vec<f64> = (0..n).map(|c| self.source[c].z).collect();

        let mut xx = vec![0.0_f64; n];
        let mut xy = vec![0.0_f64; n];
        let mut xz = vec![0.0_f64; n];

        let (ix, rx) = gs(
            &self.ldu,
            &bx,
            &mut xx,
            settings.tolerance,
            settings.max_iter,
        );
        let (iy, ry) = gs(
            &self.ldu,
            &by,
            &mut xy,
            settings.tolerance,
            settings.max_iter,
        );
        let (iz, rz) = gs(
            &self.ldu,
            &bz,
            &mut xz,
            settings.tolerance,
            settings.max_iter,
        );

        let internal = Field::from_fn(n, |c| Vector3::new(xx[c], xy[c], xz[c]));
        let boundary = mesh
            .patches
            .iter()
            .map(|p| PatchField::zero_gradient_vec(p.size))
            .collect();
        let u = VolVectorField::new(name, mesh, internal, boundary);

        let n_iters = ix.max(iy).max(iz);
        let final_res = rx.max(ry).max(rz);
        let perf = SolverPerformance {
            n_iterations: n_iters,
            final_residual: final_res,
            converged: final_res < settings.tolerance,
        };
        (u, perf)
    }

    // ── Asymmetric Krylov solves (the momentum matrix) ─────────────────────

    /// Solve each velocity component with **preconditioned BiCGStab**,
    /// cold-started from `U = 0`.
    ///
    /// The momentum matrix assembled by `fvm::div_vec(phi, U) −
    /// fvm::laplacian_vec(nu, U)` is **asymmetric** — upwind convection puts the
    /// face flux on the donor side only, so `lower[f] != upper[f]`. That rules
    /// out PCG and GAMG, and before this method the only option was the
    /// Gauss-Seidel [`solve`](Self::solve), whose sweep count grows like the
    /// matrix condition number. With the default [`KrylovOptions`] this is
    /// ILU(0)-preconditioned BiCGStab — the analogue of OpenFOAM's `PBiCGStab`
    /// with `DILU` on the `U` equation.
    ///
    /// Because the LDU coefficients are scalar and identical for all three
    /// components, this runs three independent scalar Krylov solves sharing one
    /// preconditioner build per component. The returned
    /// [`SolverPerformance`] reports the **worst** of the three: the largest
    /// iteration count and the largest relative 2-norm residual
    /// `||b − A·U||₂ / ||b||₂`, and `converged` only if all three converged.
    ///
    /// Units: `U` is a velocity `[m·s⁻¹]`; the matrix and source carry whatever
    /// units the assembling operators produced. The solve itself is
    /// dimensionless.
    pub fn solve_bicgstab(
        &self,
        name: &str,
        options: KrylovOptions,
        settings: SolverSettings,
    ) -> (VolVectorField, SolverPerformance) {
        self.solve_krylov(name, None, KrylovMethod::BiCGStab, options, settings)
    }

    /// Solve each velocity component with **restarted GMRES(m)**, cold-started
    /// from `U = 0`.
    ///
    /// See [`solve_bicgstab`](Self::solve_bicgstab) for why an asymmetric solver
    /// is needed at all. GMRES cannot break down and its residual is monotone,
    /// but it stores `options.restart` basis vectors per component; prefer it
    /// when BiCGStab reports `converged = false`.
    pub fn solve_gmres(
        &self,
        name: &str,
        options: KrylovOptions,
        settings: SolverSettings,
    ) -> (VolVectorField, SolverPerformance) {
        self.solve_krylov(name, None, KrylovMethod::Gmres, options, settings)
    }

    /// Solve each velocity component with the Krylov method named by `method`,
    /// optionally warm-started from `initial` (typically the previous time
    /// step's velocity field).
    ///
    /// The general form behind [`solve_bicgstab`](Self::solve_bicgstab) and
    /// [`solve_gmres`](Self::solve_gmres). Each component `x`, `y`, `z` of
    /// `initial` seeds the corresponding scalar solve; `None` starts from zero.
    pub fn solve_krylov(
        &self,
        name: &str,
        initial: Option<&VolVectorField>,
        method: KrylovMethod,
        options: KrylovOptions,
        settings: SolverSettings,
    ) -> (VolVectorField, SolverPerformance) {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;

        let b: [Vec<f64>; 3] = [
            (0..n).map(|c| self.source[c].x).collect(),
            (0..n).map(|c| self.source[c].y).collect(),
            (0..n).map(|c| self.source[c].z).collect(),
        ];
        let x0: Option<[Vec<f64>; 3]> = initial.map(|f| {
            [
                (0..n).map(|c| f.internal[c].x).collect(),
                (0..n).map(|c| f.internal[c].y).collect(),
                (0..n).map(|c| f.internal[c].z).collect(),
            ]
        });

        let mut sol: [Vec<f64>; 3] = [vec![0.0; n], vec![0.0; n], vec![0.0; n]];
        let mut n_iters = 0usize;
        let mut final_res = 0.0_f64;
        let mut converged = true;
        for (k, item) in sol.iter_mut().enumerate() {
            let guess = x0.as_ref().map(|g| g[k].as_slice());
            let (x, perf) = krylov_solve(&self.ldu, &b[k], guess, method, options, &settings);
            *item = x;
            n_iters = n_iters.max(perf.n_iterations);
            final_res = final_res.max(perf.final_residual);
            converged &= perf.converged;
        }

        let internal = Field::from_fn(n, |c| Vector3::new(sol[0][c], sol[1][c], sol[2][c]));
        let boundary = mesh
            .patches
            .iter()
            .map(|p| PatchField::zero_gradient_vec(p.size))
            .collect();
        let u = VolVectorField::new(name, mesh, internal, boundary);
        let perf = SolverPerformance {
            n_iterations: n_iters,
            final_residual: final_res,
            converged,
        };
        (u, perf)
    }
}

// ── Arithmetic ────────────────────────────────────────────────────────────────

impl Add for FvVectorMatrix {
    type Output = Self;
    fn add(mut self, rhs: Self) -> Self {
        for (a, b) in self.ldu.diag.iter_mut().zip(&rhs.ldu.diag) {
            *a += b;
        }
        for (a, b) in self.ldu.lower.iter_mut().zip(&rhs.ldu.lower) {
            *a += b;
        }
        for (a, b) in self.ldu.upper.iter_mut().zip(&rhs.ldu.upper) {
            *a += b;
        }
        for c in 0..self.source.len() {
            self.source[c] = self.source[c] + rhs.source[c];
        }
        self
    }
}

impl Sub for FvVectorMatrix {
    type Output = Self;
    fn sub(mut self, rhs: Self) -> Self {
        for (a, b) in self.ldu.diag.iter_mut().zip(&rhs.ldu.diag) {
            *a -= b;
        }
        for (a, b) in self.ldu.lower.iter_mut().zip(&rhs.ldu.lower) {
            *a -= b;
        }
        for (a, b) in self.ldu.upper.iter_mut().zip(&rhs.ldu.upper) {
            *a -= b;
        }
        for c in 0..self.source.len() {
            self.source[c] = self.source[c] - rhs.source[c];
        }
        self
    }
}

impl Neg for FvVectorMatrix {
    type Output = Self;
    fn neg(mut self) -> Self {
        for x in self.ldu.diag.iter_mut() {
            *x = -*x;
        }
        for x in self.ldu.lower.iter_mut() {
            *x = -*x;
        }
        for x in self.ldu.upper.iter_mut() {
            *x = -*x;
        }
        for c in 0..self.source.len() {
            self.source[c] = -self.source[c];
        }
        self
    }
}

impl AddAssign for FvVectorMatrix {
    fn add_assign(&mut self, rhs: Self) {
        for (a, b) in self.ldu.diag.iter_mut().zip(&rhs.ldu.diag) {
            *a += b;
        }
        for (a, b) in self.ldu.lower.iter_mut().zip(&rhs.ldu.lower) {
            *a += b;
        }
        for (a, b) in self.ldu.upper.iter_mut().zip(&rhs.ldu.upper) {
            *a += b;
        }
        for c in 0..self.source.len() {
            self.source[c] = self.source[c] + rhs.source[c];
        }
    }
}

impl SubAssign for FvVectorMatrix {
    fn sub_assign(&mut self, rhs: Self) {
        for (a, b) in self.ldu.diag.iter_mut().zip(&rhs.ldu.diag) {
            *a -= b;
        }
        for (a, b) in self.ldu.lower.iter_mut().zip(&rhs.ldu.lower) {
            *a -= b;
        }
        for (a, b) in self.ldu.upper.iter_mut().zip(&rhs.ldu.upper) {
            *a -= b;
        }
        for c in 0..self.source.len() {
            self.source[c] = self.source[c] - rhs.source[c];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};

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

    #[test]
    fn diagonal_system_solves_vector() {
        let m = unit_mesh();
        let mut mat = FvVectorMatrix::new(m.clone());
        mat.ldu.diag[0] = 2.0;
        mat.ldu.diag[1] = 3.0;
        mat.source[0] = Vector3::new(4.0, 6.0, 8.0);
        mat.source[1] = Vector3::new(6.0, 9.0, 12.0);
        let (u, perf) = mat.solve("U", SolverSettings::default());
        assert!(perf.converged, "residual = {}", perf.final_residual);
        assert!((u.internal[0].x - 2.0).abs() < 1e-8);
        assert!((u.internal[0].y - 3.0).abs() < 1e-8);
        assert!((u.internal[1].x - 2.0).abs() < 1e-8);
    }

    #[test]
    fn a_field_returns_diagonal() {
        let m = unit_mesh();
        let mut mat = FvVectorMatrix::new(m.clone());
        mat.ldu.diag[0] = 5.0;
        mat.ldu.diag[1] = 7.0;
        let a = mat.a_field();
        assert!((a.internal[0] - 5.0).abs() < 1e-12);
        assert!((a.internal[1] - 7.0).abs() < 1e-12);
    }
}
