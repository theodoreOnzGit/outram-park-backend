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

use crate::ldu_matrix::fv_matrix::{SolverPerformance, SolverSettings};
use crate::ldu_matrix::ldu_matrix::LduMatrix;

/// Preconditioned Conjugate Gradient solver for **symmetric** LDU matrices.
///
/// ## Preconditioner — DIC (Diagonal-based Incomplete Cholesky)
///
/// Uses OpenFOAM's default symmetric preconditioner, `DIC`
/// (`Foam::DICPreconditioner`): an incomplete Cholesky factorisation that keeps
/// only the existing matrix sparsity (no fill-in). It is a forward/backward
/// sweep over the faces using a precomputed reciprocal diagonal `rD`, and is
/// far more effective than the plain Jacobi (`M = diag(A)`) preconditioner this
/// function used previously — Jacobi-PCG iteration count grows with the mesh
/// (∝ √κ ≈ O(Nₓ)), whereas DIC dramatically flattens it.
///
/// DIC requires the faces to be in **upper-triangular order**
/// (`owner[f] < neighbour[f]`, sorted), which is how OpenFOAM `polyMesh` writes
/// internal faces and how `read_poly_mesh` loads them.
///
/// ## Warm start
///
/// `x0` is the initial guess. Pass `Some(previous_solution)` to **warm-start**
/// the solve from the last time step's field — for a transient run approaching
/// steady state the solution barely changes between steps, so the initial
/// residual is tiny and the solver converges in a handful of iterations (often
/// zero) instead of paying full convergence from `x = 0` every step. Pass
/// `None` for a cold start (`x = 0`).
///
/// ## When to use vs Gauss-Seidel
///
/// | Solver | Good for |
/// |---|---|
/// | Gauss-Seidel | Convection-dominated (asymmetric upper ≠ lower) |
/// | PCG (this) | Symmetric SPD systems — pressure Poisson (`fvm::laplacian`) |
///
/// The pressure equation assembled by `fvm::laplacian` is symmetric
/// (`upper[f] == lower[f]`), so PCG converges in O(√κ) iterations vs
/// O(κ) for Gauss-Seidel, where κ is the condition number.
pub fn conjugate_gradient(
    ldu: &LduMatrix,
    b: &[f64],
    x0: Option<&[f64]>,
    settings: &SolverSettings,
) -> (Vec<f64>, SolverPerformance) {
    let n = ldu.n_cells;
    debug_assert_eq!(b.len(), n);

    // DIC preconditioner: factorise once into the reciprocal diagonal `rD`.
    let r_diag = dic_reciprocal_diag(ldu);

    // Warm start from x0 if given, else cold start at zero.
    let mut x = match x0 {
        Some(g) => {
            debug_assert_eq!(g.len(), n);
            g.to_vec()
        }
        None => vec![0.0_f64; n],
    };
    let mut r = ldu.residual(&x, b); // r = b - A·x

    // z = M^{-1} · r  (DIC apply)
    let mut z = vec![0.0_f64; n];
    dic_precondition(ldu, &r_diag, &r, &mut z);
    let mut p = z.clone();

    let mut rz = dot(&r, &z); // r^T M^{-1} r
    let b_norm: f64 = b.iter().map(|bi| bi * bi).sum::<f64>().sqrt().max(1e-300);

    // Convergence is measured on the true (unpreconditioned) residual norm,
    // ‖r‖₂ / ‖b‖₂, so the tolerance has a mesh-independent meaning.
    let mut n_iter = 0;
    let mut final_residual = r.iter().map(|ri| ri * ri).sum::<f64>().sqrt() / b_norm;

    // A warm start may already satisfy the tolerance — return immediately and
    // skip the iteration entirely (the main win near steady state).
    if final_residual < settings.tolerance {
        return (
            x,
            SolverPerformance {
                n_iterations: 0,
                final_residual,
                converged: true,
            },
        );
    }

    for iter in 0..settings.max_iter {
        let ap = ldu.multiply(&p); // A·p
        let pap = dot(&p, &ap); // p^T A p

        if pap.abs() < 1e-300 {
            break;
        }

        let alpha = rz / pap;

        // x = x + alpha * p
        for c in 0..n {
            x[c] += alpha * p[c];
        }
        // r = r - alpha * A·p
        for c in 0..n {
            r[c] -= alpha * ap[c];
        }

        // z = M^{-1} · r  (DIC apply)
        dic_precondition(ldu, &r_diag, &r, &mut z);

        let rz_new = dot(&r, &z);
        final_residual = r.iter().map(|ri| ri * ri).sum::<f64>().sqrt() / b_norm;

        n_iter = iter + 1;
        if final_residual < settings.tolerance {
            break;
        }

        let beta = rz_new / rz;
        // p = z + beta * p
        for c in 0..n {
            p[c] = z[c] + beta * p[c];
        }

        rz = rz_new;
    }

    (
        x,
        SolverPerformance {
            n_iterations: n_iter,
            final_residual,
            converged: final_residual < settings.tolerance,
        },
    )
}

/// Precompute the DIC reciprocal diagonal `rD` for a symmetric LDU matrix.
///
/// Mirrors `Foam::DICPreconditioner::calcReciprocalD`: sweep the faces in
/// upper-triangular order subtracting `upper[f]² / rD[owner]` from the
/// neighbour's diagonal, then invert. Because the owner's diagonal is finalised
/// before any face that reads it (guaranteed by the ordering), this is an
/// in-place incomplete Cholesky with no fill.
fn dic_reciprocal_diag(ldu: &LduMatrix) -> Vec<f64> {
    let mut r_diag = ldu.diag.clone();
    for f in 0..ldu.n_internal_faces {
        let o = ldu.owner[f];
        let nb = ldu.neighbour[f];
        let d_o = r_diag[o];
        if d_o.abs() > 1e-300 {
            r_diag[nb] -= ldu.upper[f] * ldu.upper[f] / d_o;
        }
    }
    for d in r_diag.iter_mut() {
        *d = if d.abs() < 1e-300 { 1.0 } else { 1.0 / *d };
    }
    r_diag
}

/// Apply the DIC preconditioner: approximately solve `M·w = r` into `w`.
///
/// Mirrors `Foam::DICPreconditioner::precondition`: a forward sweep over the
/// faces followed by a backward sweep, using the precomputed reciprocal
/// diagonal `r_diag`. For a symmetric matrix `lower == upper`, so `upper` is
/// used for both sweeps.
fn dic_precondition(ldu: &LduMatrix, r_diag: &[f64], r: &[f64], w: &mut [f64]) {
    let n = ldu.n_cells;
    for c in 0..n {
        w[c] = r_diag[c] * r[c];
    }
    // Forward sweep (lower-triangular solve).
    for f in 0..ldu.n_internal_faces {
        let o = ldu.owner[f];
        let nb = ldu.neighbour[f];
        w[nb] -= r_diag[nb] * ldu.upper[f] * w[o];
    }
    // Backward sweep (upper-triangular solve).
    for f in (0..ldu.n_internal_faces).rev() {
        let o = ldu.owner[f];
        let nb = ldu.neighbour[f];
        w[o] -= r_diag[o] * ldu.upper[f] * w[nb];
    }
}

#[inline]
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(ai, bi)| ai * bi).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn tridiag(n: usize) -> (LduMatrix, Vec<f64>) {
        // −∇²φ = 1 on [0,1], φ(0)=φ(1)=0 → φ_exact = x(1-x)/2
        // 1D finite-difference: diag = 2/h², off-diag = -1/h², source = 1
        let h = 1.0 / (n + 1) as f64;
        let owner: Vec<usize> = (0..n - 1).collect();
        let neighbour: Vec<usize> = (1..n).collect();
        let mut m = LduMatrix::new(n, owner, neighbour);
        let coeff = 1.0 / (h * h);
        m.diag = vec![2.0 * coeff; n];
        m.upper = vec![-coeff; n - 1];
        m.lower = vec![-coeff; n - 1];
        let b = vec![1.0; n];
        (m, b)
    }

    #[test]
    fn cg_identity() {
        let mut m = LduMatrix::new(3, vec![], vec![]);
        m.diag = vec![2.0, 3.0, 4.0];
        let b = vec![4.0, 9.0, 8.0];
        let settings = SolverSettings {
            tolerance: 1e-10,
            max_iter: 100,
        };
        let (x, perf) = conjugate_gradient(&m, &b, None, &settings);
        assert!(perf.converged, "residual = {}", perf.final_residual);
        assert_relative_eq!(x[0], 2.0, epsilon = 1e-8);
        assert_relative_eq!(x[1], 3.0, epsilon = 1e-8);
        assert_relative_eq!(x[2], 2.0, epsilon = 1e-8);
    }

    #[test]
    fn cg_solves_1d_poisson() {
        let n = 50;
        let h = 1.0 / (n + 1) as f64;
        let (m, b) = tridiag(n);
        let settings = SolverSettings {
            tolerance: 1e-10,
            max_iter: 1000,
        };
        let (x, perf) = conjugate_gradient(&m, &b, None, &settings);
        assert!(
            perf.converged,
            "did not converge: residual = {}",
            perf.final_residual
        );
        // Check against exact solution φ = x*(1-x)/2 at interior points
        for i in 0..n {
            let xi = (i + 1) as f64 * h;
            let exact = xi * (1.0 - xi) / 2.0;
            assert_relative_eq!(x[i], exact, epsilon = h * h);
        }
    }

    #[test]
    fn cg_faster_than_gs_on_symmetric() {
        // CG should converge in at most n iterations on an n×n SPD system
        let n = 20;
        let (m, b) = tridiag(n);
        let settings = SolverSettings {
            tolerance: 1e-10,
            max_iter: 500,
        };
        let (_, perf) = conjugate_gradient(&m, &b, None, &settings);
        assert!(perf.converged);
        // CG on a tridiagonal SPD should need << n iterations with preconditioning
        assert!(
            perf.n_iterations <= n,
            "expected <= {} iters, got {}",
            n,
            perf.n_iterations
        );
    }
}
