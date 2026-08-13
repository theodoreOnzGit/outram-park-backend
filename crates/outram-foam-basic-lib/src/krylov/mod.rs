// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! Asymmetric Krylov iterative solvers and preconditioners for sparse `A x = b`.
//!
//! This module complements the crate's existing SPD-only machinery (DIC-PCG and
//! GAMG in [`crate::ldu_matrix::solvers`]) with the **nonsymmetric** iterative
//! solvers a Newton–Krylov subsurface-flow solver needs, where the Jacobian is
//! not symmetric:
//!
//! - [`bicgstab`](crate::krylov::bicgstab()) — preconditioned BiCGStab: fixed work/storage per iteration,
//!   breakdown-guarded.
//! - [`gmres`](crate::krylov::gmres()) — restarted, right-preconditioned GMRES(m): residual-minimising,
//!   robust, `O(m)` storage.
//!
//! and three preconditioners dispatched by the [`Preconditioner`](crate::krylov::Preconditioner) enum (never
//! trait objects, per the workspace design rules):
//!
//! - [`Preconditioner::identity`](crate::krylov::Preconditioner::identity) — no preconditioning (`M = I`).
//! - [`Preconditioner::jacobi`](crate::krylov::Preconditioner::jacobi) — diagonal scaling; always applicable.
//! - [`Preconditioner::ilu0`](crate::krylov::Preconditioner::ilu0) — genuine ILU(0) incomplete factorisation.
//!
//! # Matrix representation and conventions
//!
//! All solvers act on [`crate::ldu_matrix::LduMatrix`], the crate's face-addressed
//! sparse matrix, and use only its `multiply` (SpMV) and `residual` kernels. The
//! system size `n` is `LduMatrix::n_cells`; all right-hand-side, guess, and
//! solution slices have length `n`. Every quantity here is a dimensionless `f64`:
//! a Krylov subspace mixes residuals, search directions and increments that share
//! no single physical dimension, so no `uom` typing is applied — apply units at
//! the field/equation layer that assembles the matrix.
//!
//! # Execution backend
//!
//! Both solvers run on the hybrid [`ComputeBackend`], driving the kernels in
//! [`crate::ldu_matrix::parallel`]. Each has **one** implementation with the
//! backend as a parameter — [`bicgstab_prepared`] and [`gmres_prepared`], which
//! take a [`HybridLdu`](crate::ldu_matrix::parallel::HybridLdu) so the
//! cell-gather index is built once per mesh rather than once per solve — plus a
//! convenience adapter ([`bicgstab`], [`gmres`]) for a caller holding a bare
//! [`LduMatrix`], which builds the index and runs on
//! [`ComputeBackend::Serial`]. They are not a serial/parallel pair: they differ
//! in who owns the index, and the backend is a parameter of both.
//!
//! Whole-solve wall clock on 4 logical cores, release, `--features parallel`,
//! 512 000 cells, measured 2026-08-13 over four independent runs: **2.4-2.7x**
//! with Jacobi preconditioning, **~1.5x** with ILU(0) — the gap being ILU(0)'s
//! inherently sequential triangular solves, measured at 29-46% of the *serial*
//! solve, which caps it at 1.7-2.1x on 4 cores. Below roughly 13 000 cells the
//! parallel path **loses** (0.6-0.8x at 4 096 cells). Full tables, methodology
//! and limitations are on the benchmarks in `hybrid_tests` and summarised on
//! [`bicgstab_prepared`].
//!
//! **Backend parity is bitwise**, not tolerance-based: `Serial` and `CpuMulti`
//! produce identical iterates, identical residual histories and identical
//! iteration counts at any thread count, because every kernel underneath carries
//! that guarantee individually.
//!
//! # Convergence
//!
//! The stopping test for both solvers is the **relative** residual
//! `||b − A x||₂ / ||b||₂ ≤ tolerance`. The reported `final_residual` is always the
//! *true* residual of the returned iterate (recomputed from `A` and `b`), not an
//! internal estimate. A right-hand side that is exactly zero returns `x = 0`,
//! `converged = true`, `0` iterations.
//!
//! # Example
//!
//! ```rust
//! use outram_foam_basic_lib::ldu_matrix::LduMatrix;
//! use outram_foam_basic_lib::krylov::{bicgstab, Preconditioner, KrylovSettings};
//!
//! // 3-cell chain: cells 0-1 and 1-2 share a face each.
//! let mut a = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
//! a.diag = vec![4.0, 4.0, 4.0];
//! a.lower = vec![-1.0, -1.0];
//! a.upper = vec![-1.0, -1.0];
//! let b = vec![1.0, 2.0, 3.0];
//!
//! let precond = Preconditioner::jacobi(&a);
//! let settings = KrylovSettings::default();
//! let (x, result) = bicgstab(&a, &b, None, &precond, &settings);
//!
//! assert!(result.converged);
//! // Verify: A x ≈ b.
//! let ax = a.multiply(&x);
//! for i in 0..3 {
//!     assert!((ax[i] - b[i]).abs() < 1e-6);
//! }
//! ```

mod bicgstab;
mod gmres;
mod preconditioner;
pub mod vecops;

#[cfg(test)]
mod hybrid_tests;

pub use bicgstab::{bicgstab, bicgstab_prepared};
pub use gmres::{gmres, gmres_prepared};
pub use preconditioner::{Ilu0Preconditioner, JacobiPreconditioner};

use crate::compute::ComputeBackend;
use crate::ldu_matrix::LduMatrix;

/// Iteration controls shared by [`bicgstab`] and [`gmres`].
///
/// All fields are plain scalars with no units.
#[derive(Debug, Clone, Copy)]
pub struct KrylovSettings {
    /// Relative convergence tolerance on `||b − A x||₂ / ||b||₂`. Must be `> 0`;
    /// typical range `1e-12 .. 1e-4`. Default `1e-8`.
    pub tolerance: f64,
    /// Maximum total number of iterations (matrix–vector products) before the
    /// solver returns unconverged. Default `1000`.
    pub max_iter: usize,
    /// GMRES restart length `m` — the Krylov subspace dimension per outer cycle,
    /// trading memory (`O(m·n)`) against convergence robustness. Ignored by
    /// BiCGStab. `0` means "no restart" (`m = max_iter`). Default `30`.
    pub restart: usize,
}

impl Default for KrylovSettings {
    /// Defaults: `tolerance = 1e-8`, `max_iter = 1000`, `restart = 30`.
    fn default() -> Self {
        Self {
            tolerance: 1.0e-8,
            max_iter: 1000,
            restart: 30,
        }
    }
}

/// Outcome of a Krylov solve.
///
/// All fields are plain scalars with no units.
#[derive(Debug, Clone, Copy)]
pub struct KrylovResult {
    /// Number of iterations (matrix–vector products) actually performed.
    pub n_iterations: usize,
    /// The **true** relative residual `||b − A x||₂ / ||b||₂` of the returned
    /// iterate, recomputed from `A` and `b` (dimensionless, `>= 0`).
    pub final_residual: f64,
    /// `true` iff `final_residual <= settings.tolerance`.
    pub converged: bool,
}

/// Preconditioner `M^{-1} ≈ A^{-1}`, dispatched by enum (no trait objects).
///
/// A preconditioner turns a residual `r` into `z = M^{-1} r`, an approximate
/// error, which the Krylov solvers use to accelerate convergence. Construct one
/// from the system matrix with [`Preconditioner::jacobi`] or
/// [`Preconditioner::ilu0`], or use [`Preconditioner::identity`] for none.
pub enum Preconditioner {
    /// No preconditioning: `M = I`, so `z = r`.
    Identity,
    /// Diagonal (Jacobi) scaling: `z = r / diag(A)`.
    Jacobi(JacobiPreconditioner),
    /// ILU(0) incomplete factorisation: `z = (LU)^{-1} r`.
    Ilu0(Ilu0Preconditioner),
}

impl Preconditioner {
    /// Identity preconditioner (`M = I`) — equivalent to no preconditioning.
    pub fn identity() -> Self {
        Preconditioner::Identity
    }

    /// Build a Jacobi (reciprocal-diagonal) preconditioner from `a`.
    ///
    /// Cannot break down: a zero diagonal entry is treated as `1.0`. The
    /// guaranteed-robust choice when ILU(0) is not warranted.
    pub fn jacobi(a: &LduMatrix) -> Self {
        Preconditioner::Jacobi(JacobiPreconditioner::new(a))
    }

    /// Build an ILU(0) preconditioner from `a` (same sparsity pattern as `A`).
    ///
    /// A genuine incomplete LU factorisation with a small-pivot guard; see
    /// [`Ilu0Preconditioner`]. Typically far stronger than Jacobi on
    /// diagonally-dominant / mildly ill-conditioned systems.
    pub fn ilu0(a: &LduMatrix) -> Self {
        Preconditioner::Ilu0(Ilu0Preconditioner::new(a))
    }

    /// Apply the preconditioner serially: write `z = M^{-1} r`.
    ///
    /// `r` and `z` must both have length `n_cells`. For [`Preconditioner::Identity`]
    /// this copies `r` into `z`. Equivalent to [`Self::apply_on`] with
    /// [`ComputeBackend::Serial`].
    pub fn apply(&self, r: &[f64], z: &mut [f64]) {
        self.apply_on(r, z, ComputeBackend::Serial);
    }

    /// Apply the preconditioner on the chosen [`ComputeBackend`]: write
    /// `z = M^{-1} r`.
    ///
    /// # How much each variant actually parallelises
    ///
    /// | Variant | Backend honoured? | Why |
    /// |---|---|---|
    /// | [`Identity`](Self::Identity) | no — `copy_from_slice` | already a single memcpy; there is nothing to thread |
    /// | [`Jacobi`](Self::Jacobi) | **yes** | one independent multiply per cell — embarrassingly parallel |
    /// | [`Ilu0`](Self::Ilu0) | no — always serial | the triangular solves have a loop-carried dependence; see [`Ilu0Preconditioner::apply_on`] |
    ///
    /// This table is the honest answer to "did the preconditioners move onto the
    /// backend?": one of the three did, and the reason the other two did not is
    /// structural rather than unfinished work.
    ///
    /// # Determinism
    ///
    /// All three variants are bitwise identical across backends and thread
    /// counts, and bitwise identical to [`Self::apply`]. Two are serial anyway,
    /// and Jacobi is element-wise, so no summation is reassociated. A caller may
    /// therefore switch backend without perturbing a solver's iteration count or
    /// residual history through the preconditioner.
    ///
    /// # Arguments
    ///
    /// - `r` — residual to precondition, length `n_cells`.
    /// - `z` — output buffer, length `n_cells`. Fully overwritten.
    /// - `backend` — requested execution backend.
    pub fn apply_on(&self, r: &[f64], z: &mut [f64], backend: ComputeBackend) {
        debug_assert_eq!(r.len(), z.len(), "Preconditioner::apply length mismatch");
        match self {
            Preconditioner::Identity => z.copy_from_slice(r),
            Preconditioner::Jacobi(p) => preconditioner::jacobi_apply(p, r, z, backend),
            Preconditioner::Ilu0(p) => preconditioner::ilu0_apply(p, r, z, backend),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::SquareMatrix;

    /// Materialise an LduMatrix as a dense SquareMatrix (for cross-checking).
    fn dense_of(a: &LduMatrix) -> SquareMatrix {
        let mut m = SquareMatrix::new(a.n_cells);
        for i in 0..a.n_cells {
            m.set(i, i, a.diag[i]);
        }
        for f in 0..a.n_internal_faces {
            let o = a.owner[f];
            let n = a.neighbour[f];
            m.set(o, n, a.upper[f]);
            m.set(n, o, a.lower[f]);
        }
        m
    }

    /// Symmetric SPD tridiagonal: diag d, off-diag e (lower==upper).
    fn spd_tridiag(n: usize, d: f64, e: f64) -> LduMatrix {
        let mut owner = Vec::new();
        let mut neigh = Vec::new();
        for i in 0..n - 1 {
            owner.push(i);
            neigh.push(i + 1);
        }
        let mut a = LduMatrix::new(n, owner, neigh);
        for i in 0..n {
            a.diag[i] = d;
        }
        for f in 0..a.n_internal_faces {
            a.lower[f] = e;
            a.upper[f] = e;
        }
        a
    }

    /// Nonsymmetric tridiagonal: distinct lower and upper off-diagonals.
    fn nonsym_tridiag(n: usize, d: f64, lo: f64, up: f64) -> LduMatrix {
        let mut owner = Vec::new();
        let mut neigh = Vec::new();
        for i in 0..n - 1 {
            owner.push(i);
            neigh.push(i + 1);
        }
        let mut a = LduMatrix::new(n, owner, neigh);
        for i in 0..n {
            a.diag[i] = d;
        }
        for f in 0..a.n_internal_faces {
            a.lower[f] = lo;
            a.upper[f] = up;
        }
        a
    }

    fn rel_err(x: &[f64], y: &[f64]) -> f64 {
        let mut num = 0.0;
        let mut den = 0.0;
        for i in 0..x.len() {
            num += (x[i] - y[i]).powi(2);
            den += y[i].powi(2);
        }
        (num / den).sqrt()
    }

    // (1) Small SPD system, known solution, both solvers.
    #[test]
    fn spd_known_solution() {
        let n = 4;
        let a = spd_tridiag(n, 2.0, -1.0);
        // Choose x_true, form b = A x.
        let x_true = vec![1.0, 2.0, 3.0, 4.0];
        let b = a.multiply(&x_true);
        let settings = KrylovSettings {
            tolerance: 1e-10,
            max_iter: 200,
            restart: 30,
        };

        let (xb, rb) = bicgstab(&a, &b, None, &Preconditioner::identity(), &settings);
        assert!(rb.converged, "bicgstab did not converge: {:?}", rb);
        assert!(rel_err(&xb, &x_true) < 1e-7);

        let (xg, rg) = gmres(&a, &b, None, &Preconditioner::identity(), &settings);
        assert!(rg.converged, "gmres did not converge: {:?}", rg);
        assert!(rel_err(&xg, &x_true) < 1e-7);
    }

    // (2) Nonsymmetric system solved by both, cross-checked vs dense LU.
    #[test]
    fn nonsymmetric_vs_dense_lu() {
        let n = 6;
        let a = nonsym_tridiag(n, 4.0, -1.0, -2.0); // lower != upper
        let b: Vec<f64> = (0..n).map(|i| (i as f64) - 2.5).collect();

        // Dense reference.
        let dense = dense_of(&a);
        let x_ref = dense.solve(&b).expect("dense LU failed");

        let settings = KrylovSettings {
            tolerance: 1e-8,
            max_iter: 500,
            restart: 30,
        };

        for precond in [
            Preconditioner::identity(),
            Preconditioner::jacobi(&a),
            Preconditioner::ilu0(&a),
        ] {
            let (xb, rb) = bicgstab(&a, &b, None, &precond, &settings);
            assert!(rb.converged, "bicgstab unconverged: {:?}", rb);
            assert!(
                rel_err(&xb, &x_ref) < 1e-6,
                "bicgstab mismatch vs dense LU: {}",
                rel_err(&xb, &x_ref)
            );

            let (xg, rg) = gmres(&a, &b, None, &precond, &settings);
            assert!(rg.converged, "gmres unconverged: {:?}", rg);
            assert!(
                rel_err(&xg, &x_ref) < 1e-6,
                "gmres mismatch vs dense LU: {}",
                rel_err(&xg, &x_ref)
            );
        }
    }

    // (3) Preconditioning helps (or at least still converges) on an
    //     ill-conditioned-ish system: ILU(0) should not need MORE iterations
    //     than Identity, and usually far fewer.
    #[test]
    fn preconditioning_reduces_iterations() {
        let n = 40;
        // Mildly ill-conditioned SPD-ish, diagonally dominant tridiagonal.
        let a = nonsym_tridiag(n, 2.05, -1.0, -1.0);
        let b = vec![1.0; n];
        let settings = KrylovSettings {
            tolerance: 1e-8,
            max_iter: 2000,
            restart: 50,
        };

        let (_, r_id) = bicgstab(&a, &b, None, &Preconditioner::identity(), &settings);
        let (_, r_jac) = bicgstab(&a, &b, None, &Preconditioner::jacobi(&a), &settings);
        let (_, r_ilu) = bicgstab(&a, &b, None, &Preconditioner::ilu0(&a), &settings);

        assert!(r_id.converged && r_jac.converged && r_ilu.converged);
        // ILU(0) must not be worse than identity, and should help here.
        assert!(
            r_ilu.n_iterations <= r_id.n_iterations,
            "ILU(0) {} vs Identity {} iters",
            r_ilu.n_iterations,
            r_id.n_iterations
        );

        // Same check for GMRES.
        let (_, g_id) = gmres(&a, &b, None, &Preconditioner::identity(), &settings);
        let (_, g_ilu) = gmres(&a, &b, None, &Preconditioner::ilu0(&a), &settings);
        assert!(g_id.converged && g_ilu.converged);
        assert!(g_ilu.n_iterations <= g_id.n_iterations);
    }

    // (4) Zero RHS shortcut.
    #[test]
    fn zero_rhs() {
        let a = spd_tridiag(5, 3.0, -1.0);
        let b = vec![0.0; 5];
        let s = KrylovSettings::default();
        let (xb, rb) = bicgstab(&a, &b, None, &Preconditioner::identity(), &s);
        assert!(rb.converged && rb.n_iterations == 0 && xb.iter().all(|&v| v == 0.0));
        let (xg, rg) = gmres(&a, &b, None, &Preconditioner::identity(), &s);
        assert!(rg.converged && rg.n_iterations == 0 && xg.iter().all(|&v| v == 0.0));
    }

    // (5) Nonzero initial guess is honoured.
    #[test]
    fn initial_guess() {
        let a = spd_tridiag(4, 2.0, -1.0);
        let x_true = vec![0.5, 1.5, -2.0, 3.0];
        let b = a.multiply(&x_true);
        let s = KrylovSettings {
            tolerance: 1e-10,
            max_iter: 200,
            restart: 30,
        };
        let guess = x_true.clone();
        let (x, r) = gmres(&a, &b, Some(&guess), &Preconditioner::ilu0(&a), &s);
        assert!(r.converged);
        assert!(rel_err(&x, &x_true) < 1e-7);
    }
}
