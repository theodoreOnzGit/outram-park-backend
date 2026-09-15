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

//! Bridge from the finite-volume solver settings to the asymmetric Krylov
//! solvers in [`crate::krylov`].
//!
//! [`crate::krylov`] holds solver *kernels* (BiCGStab, restarted GMRES) that
//! speak plain `LduMatrix` + `&[f64]`. The finite-volume layer
//! ([`FvMatrix`](crate::ldu_matrix::FvMatrix),
//! [`FvVectorMatrix`](crate::ldu_matrix::FvVectorMatrix)) speaks
//! [`SolverSettings`] / [`SolverPerformance`]. This module is the thin adapter
//! between the two, plus the two small selection enums that let a caller choose
//! the method and the preconditioner **by value, never by trait object**
//! (workspace design rule).
//!
//! # Why this matters physically
//!
//! Any equation carrying a convection term — momentum, energy, or a transported
//! scalar — assembles an **asymmetric** matrix (`lower[f] != upper[f]`), because
//! upwinding puts the flux on the donor side only. The crate's symmetric
//! machinery (DIC-PCG, GAMG) is therefore inapplicable to those systems, and
//! before this module the only fallback was plain Gauss-Seidel, whose iteration
//! count grows like the condition number `O(kappa)`. BiCGStab/GMRES with an
//! ILU(0) preconditioner is the direct analogue of OpenFOAM's `PBiCGStab` with
//! `DILU`, and converges far faster on the same systems. See
//! `tests/krylov_convection_diffusion.rs` for measured iteration counts.
//!
//! # Units
//!
//! The linear algebra is dimensionless. `A`, `b` and `x` carry whatever units the
//! assembling operator gave them (e.g. for `fvm::laplacian(gamma, T)` the source
//! is `[gamma]·[T]·m`); the solver only ever forms ratios, so no `uom` typing is
//! applied here. Apply units at the field/equation layer.

use std::sync::Arc;

use crate::compute::ComputeBackend;
use crate::krylov::{bicgstab_prepared, gmres_prepared, KrylovSettings, Preconditioner};
use crate::ldu_matrix::fv_matrix::{SolverPerformance, SolverSettings};
use crate::ldu_matrix::ldu_matrix::LduMatrix;
use crate::ldu_matrix::parallel::HybridLdu;

/// Which preconditioner `M^{-1} ~ A^{-1}` a Krylov solve should build from the
/// matrix.
///
/// This is a *selection* enum: it carries no data, so it is `Copy` and can sit
/// in a settings struct. The built preconditioner itself is
/// [`crate::krylov::Preconditioner`]. Dispatch is by enum, never a trait object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreconditionerKind {
    /// No preconditioning (`M = I`). Cheapest per iteration, most iterations.
    /// Use only as a baseline or when the matrix is already well conditioned.
    None,
    /// Diagonal (Jacobi) scaling, `z = r / diag(A)`. Cannot break down; costs one
    /// divide per cell per iteration. A good default for a strongly
    /// diagonally-dominant matrix (small time step, low Peclet number).
    Jacobi,
    /// ILU(0) incomplete factorisation on the matrix's own sparsity pattern —
    /// the analogue of OpenFOAM's `DILU`. Typically several times fewer
    /// iterations than Jacobi on convection-dominated systems, at the cost of one
    /// forward/backward sweep per iteration. **Default.**
    #[default]
    Ilu0,
}

impl PreconditionerKind {
    /// Build the concrete preconditioner for `a`.
    ///
    /// `a` must be the same matrix the solve is about to be run on — ILU(0) in
    /// particular factorises `a`'s coefficients, so reusing a preconditioner
    /// built from a different matrix silently degrades convergence.
    pub fn build(self, a: &LduMatrix) -> Preconditioner {
        match self {
            PreconditionerKind::None => Preconditioner::identity(),
            PreconditionerKind::Jacobi => Preconditioner::jacobi(a),
            PreconditionerKind::Ilu0 => Preconditioner::ilu0(a),
        }
    }
}

/// Which asymmetric Krylov method to run.
///
/// Both handle `lower[f] != upper[f]`; neither requires symmetry or positive
/// definiteness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KrylovMethod {
    /// Preconditioned BiCGStab — constant work and storage per iteration.
    /// The default, and the right first choice for a finite-volume momentum or
    /// scalar-transport matrix. Can break down on strongly nonnormal systems, in
    /// which case the solve returns `converged = false` with the best iterate
    /// found rather than garbage.
    #[default]
    BiCGStab,
    /// Restarted, right-preconditioned GMRES(m) — minimises the residual over
    /// the Krylov subspace, so its residual history is monotone and it cannot
    /// break down, but it stores `m` basis vectors (`O(m·n_cells)` memory).
    /// Prefer it when BiCGStab stalls or breaks down.
    Gmres,
}

/// Extra controls a Krylov solve needs beyond [`SolverSettings`].
///
/// Kept separate from `SolverSettings` deliberately: `SolverSettings` is shared
/// with Gauss-Seidel, PCG and GAMG, and adding fields to it would break every
/// existing struct-literal construction.
///
/// All fields are dimensionless.
#[derive(Debug, Clone, Copy)]
pub struct KrylovOptions {
    /// Preconditioner to build from the matrix. Default
    /// [`PreconditionerKind::Ilu0`].
    pub preconditioner: PreconditionerKind,
    /// GMRES restart length `m` — the Krylov subspace dimension per outer cycle.
    /// Larger `m` converges in fewer total inner iterations but costs `O(m·n)`
    /// memory. Ignored by [`KrylovMethod::BiCGStab`]. `0` means "no restart"
    /// (`m = max_iter`). Default `30`.
    pub restart: usize,
    /// Where the solver's kernels run — the sparse products, the inner products,
    /// the vector updates and the Jacobi preconditioner application.
    ///
    /// This is the field through which the whole finite-volume layer reaches the
    /// hybrid backend: [`FvMatrix::solve_bicgstab`](crate::ldu_matrix::FvMatrix::solve_bicgstab)
    /// and its siblings take a `KrylovOptions`, so setting this is the only
    /// change a caller needs to make.
    ///
    /// **Default [`ComputeBackend::Serial`]**, deliberately: `Serial` is this
    /// workspace's documented oracle and the default must be the trusted path.
    /// Because every kernel the solvers use is bitwise identical between
    /// `Serial` and [`ComputeBackend::CpuMulti`], switching this changes wall
    /// clock only — not the answer, not the residual history, not the iteration
    /// count. A backend whose Cargo feature is off, or whose hardware is absent,
    /// degrades instead of failing.
    pub backend: ComputeBackend,
}

impl Default for KrylovOptions {
    /// Defaults: ILU(0) preconditioning, GMRES restart `m = 30`, and the
    /// [`ComputeBackend::Serial`] oracle backend.
    fn default() -> Self {
        Self {
            preconditioner: PreconditionerKind::Ilu0,
            restart: 30,
            backend: ComputeBackend::Serial,
        }
    }
}

impl KrylovOptions {
    /// Options using the given preconditioner and the default restart (`30`) and
    /// backend ([`ComputeBackend::Serial`]).
    pub fn with_preconditioner(preconditioner: PreconditionerKind) -> Self {
        Self {
            preconditioner,
            ..Self::default()
        }
    }

    /// Options using the given execution backend, otherwise the defaults
    /// (ILU(0), restart 30).
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::compute::ComputeBackend;
    /// use outram_foam_basic_lib::ldu_matrix::KrylovOptions;
    ///
    /// let opts = KrylovOptions::on_backend(ComputeBackend::CpuMulti);
    /// assert_eq!(opts.backend, ComputeBackend::CpuMulti);
    /// assert_eq!(opts.restart, 30);
    /// ```
    pub fn on_backend(backend: ComputeBackend) -> Self {
        Self {
            backend,
            ..Self::default()
        }
    }
}

/// Solve `A·x = b` with an asymmetric Krylov method, reporting in the
/// finite-volume layer's [`SolverPerformance`] form.
///
/// This is the single entry point that
/// [`FvMatrix::solve_krylov`](crate::ldu_matrix::FvMatrix::solve_krylov) and
/// [`FvVectorMatrix::solve_krylov`](crate::ldu_matrix::FvVectorMatrix::solve_krylov)
/// call; use it directly when you already hold a raw [`LduMatrix`].
///
/// # Arguments
/// - `a` — the sparse system matrix. May be asymmetric (`lower != upper`);
///   symmetric matrices also work but PCG/GAMG are cheaper for those.
/// - `b` — right-hand side, length `a.n_cells`.
/// - `x0` — optional initial guess (e.g. the previous time step's field);
///   `None` starts from zero.
/// - `method` — BiCGStab or GMRES(m).
/// - `options` — preconditioner choice and GMRES restart length.
/// - `settings` — `tolerance` and `max_iter`, shared with the other solvers.
///
/// # Convergence measure
///
/// `SolverPerformance::final_residual` is the **true relative 2-norm residual**
/// `||b − A·x||₂ / ||b||₂` of the returned iterate, recomputed from `a` and `b`
/// — the same measure
/// [`conjugate_gradient`](crate::ldu_matrix::solvers::conjugate_gradient()) reports, and
/// **not** the L1-scaled [`LduMatrix::normalised_residual`] that
/// [`gauss_seidel`](crate::ldu_matrix::solvers::gauss_seidel()) reports. When comparing
/// against Gauss-Seidel, recompute one common measure rather than comparing the
/// two reported numbers directly.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::ldu_matrix::LduMatrix;
/// use outram_foam_basic_lib::ldu_matrix::{
///     krylov_solve, KrylovMethod, KrylovOptions, PreconditionerKind, SolverSettings,
/// };
///
/// // Asymmetric 3-cell chain: upper != lower (an upwinded convection stencil).
/// let mut a = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
/// a.diag = vec![4.0, 4.0, 4.0];
/// a.lower = vec![-2.0, -2.0];
/// a.upper = vec![-1.0, -1.0];
/// let b = vec![1.0, 2.0, 3.0];
///
/// let (x, perf) = krylov_solve(
///     &a,
///     &b,
///     None,
///     KrylovMethod::BiCGStab,
///     KrylovOptions::with_preconditioner(PreconditionerKind::Ilu0),
///     &SolverSettings::default(),
/// );
/// assert!(perf.converged);
///
/// let ax = a.multiply(&x);
/// for i in 0..3 {
///     assert!((ax[i] - b[i]).abs() < 1e-6);
/// }
/// ```
pub fn krylov_solve(
    a: &LduMatrix,
    b: &[f64],
    x0: Option<&[f64]>,
    method: KrylovMethod,
    options: KrylovOptions,
    settings: &SolverSettings,
) -> (Vec<f64>, SolverPerformance) {
    let precond = options.preconditioner.build(a);
    let ldu = HybridLdu::new(Arc::new(a.clone()));
    krylov_solve_prepared(&ldu, b, x0, method, &precond, options, settings)
}

/// Solve `A·x = b` with an asymmetric Krylov method, reusing a caller-owned
/// cell-gather index and a caller-owned preconditioner.
///
/// The form [`krylov_solve`] delegates to, and the one a solver loop should call.
/// It differs only in **what the caller has already prepared**, not in where it
/// runs — the backend is [`KrylovOptions::backend`] in both cases, so this is not
/// a parallel sibling of a serial function.
///
/// # Why prepare anything
///
/// [`krylov_solve`] pays two costs on every call that a repeated solve should not:
///
/// - it clones `a` into an [`Arc`] and rebuilds the
///   `O(n_cells + n_internal_faces)` cell-gather index
///   ([`LduTopology`](crate::ldu_matrix::parallel::LduTopology)), and
/// - it rebuilds the preconditioner, which for
///   [`PreconditionerKind::Ilu0`] is a full incomplete factorisation.
///
/// A finite-volume solver reassembles coefficients every outer iteration while
/// the **mesh addressing never changes**, so it should build the index once and
/// refresh it with [`HybridLdu::with_matrix`], which is an addressing check
/// rather than a rebuild. The preconditioner does have to be rebuilt whenever the
/// coefficients change — ILU(0) factorises them — so it is passed in explicitly
/// rather than silently reused, which would degrade convergence without any
/// visible symptom.
///
/// # Arguments
///
/// - `ldu` — the prepared sparse system.
/// - `b` — right-hand side, length `n_cells`.
/// - `x0` — optional initial guess.
/// - `method` — BiCGStab or GMRES(m).
/// - `precond` — preconditioner built from **these** coefficients.
/// - `options` — GMRES restart length and the execution backend.
///   [`KrylovOptions::preconditioner`] is **ignored** here, because the
///   preconditioner is supplied directly.
/// - `settings` — `tolerance` and `max_iter`.
///
/// # Returns
///
/// As [`krylov_solve`].
///
/// # Example
///
/// ```rust
/// use std::sync::Arc;
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::krylov::Preconditioner;
/// use outram_foam_basic_lib::ldu_matrix::parallel::HybridLdu;
/// use outram_foam_basic_lib::ldu_matrix::{
///     krylov_solve_prepared, KrylovMethod, KrylovOptions, LduMatrix, SolverSettings,
/// };
///
/// let mut a = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
/// a.diag = vec![4.0, 4.0, 4.0];
/// a.lower = vec![-2.0, -2.0];
/// a.upper = vec![-1.0, -1.0];
/// let b = vec![1.0, 2.0, 3.0];
///
/// let precond = Preconditioner::ilu0(&a);
/// let ldu = HybridLdu::new(Arc::new(a));
///
/// let (x, perf) = krylov_solve_prepared(
///     &ldu,
///     &b,
///     None,
///     KrylovMethod::BiCGStab,
///     &precond,
///     KrylovOptions::on_backend(ComputeBackend::CpuMulti),
///     &SolverSettings::default(),
/// );
/// assert!(perf.converged);
///
/// let ax = ldu.spmv(&x, ComputeBackend::Serial);
/// for i in 0..3 {
///     assert!((ax[i] - b[i]).abs() < 1e-6);
/// }
/// ```
pub fn krylov_solve_prepared(
    ldu: &HybridLdu,
    b: &[f64],
    x0: Option<&[f64]>,
    method: KrylovMethod,
    precond: &Preconditioner,
    options: KrylovOptions,
    settings: &SolverSettings,
) -> (Vec<f64>, SolverPerformance) {
    let ks = KrylovSettings {
        tolerance: settings.tolerance,
        max_iter: settings.max_iter,
        restart: options.restart,
    };
    let (x, result) = match method {
        KrylovMethod::BiCGStab => bicgstab_prepared(ldu, b, x0, precond, &ks, options.backend),
        KrylovMethod::Gmres => gmres_prepared(ldu, b, x0, precond, &ks, options.backend),
    };
    let perf = SolverPerformance {
        n_iterations: result.n_iterations,
        final_residual: result.final_residual,
        converged: result.converged,
    };
    (x, perf)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Asymmetric tridiagonal `A` with distinct lower/upper off-diagonals —
    /// the algebraic shape of an upwinded 1-D convection-diffusion operator.
    fn asym_tridiag(n: usize, d: f64, lo: f64, up: f64) -> LduMatrix {
        let owner: Vec<usize> = (0..n - 1).collect();
        let neigh: Vec<usize> = (1..n).collect();
        let mut a = LduMatrix::new(n, owner, neigh);
        a.diag = vec![d; n];
        a.lower = vec![lo; n - 1];
        a.upper = vec![up; n - 1];
        a
    }

    #[test]
    fn every_method_preconditioner_pair_solves_an_asymmetric_system() {
        let n = 50;
        let a = asym_tridiag(n, 2.10, -1.2, -0.8);
        let x_true: Vec<f64> = (0..n).map(|i| ((i as f64) * 0.37).sin()).collect();
        let b = a.multiply(&x_true);
        let settings = SolverSettings {
            tolerance: 1e-10,
            max_iter: 2000,
        };

        for method in [KrylovMethod::BiCGStab, KrylovMethod::Gmres] {
            for pk in [
                PreconditionerKind::None,
                PreconditionerKind::Jacobi,
                PreconditionerKind::Ilu0,
            ] {
                let (x, perf) = krylov_solve(
                    &a,
                    &b,
                    None,
                    method,
                    KrylovOptions {
                        preconditioner: pk,
                        restart: 40,
                        ..KrylovOptions::default()
                    },
                    &settings,
                );
                assert!(
                    perf.converged,
                    "{:?}/{:?} unconverged: {:?}",
                    method, pk, perf
                );
                let err: f64 = x
                    .iter()
                    .zip(&x_true)
                    .map(|(a, b)| (a - b) * (a - b))
                    .sum::<f64>()
                    .sqrt();
                assert!(err < 1e-6, "{:?}/{:?} solution error {}", method, pk, err);
            }
        }
    }

    #[test]
    fn ilu0_needs_no_more_iterations_than_unpreconditioned() {
        let n = 120;
        let a = asym_tridiag(n, 2.02, -1.1, -0.9);
        let b = vec![1.0; n];
        let settings = SolverSettings {
            tolerance: 1e-9,
            max_iter: 5000,
        };
        let opts = |p| KrylovOptions {
            preconditioner: p,
            restart: 30,
            ..KrylovOptions::default()
        };
        let (_, none) = krylov_solve(
            &a,
            &b,
            None,
            KrylovMethod::BiCGStab,
            opts(PreconditionerKind::None),
            &settings,
        );
        let (_, ilu) = krylov_solve(
            &a,
            &b,
            None,
            KrylovMethod::BiCGStab,
            opts(PreconditionerKind::Ilu0),
            &settings,
        );
        assert!(none.converged && ilu.converged);
        assert!(
            ilu.n_iterations <= none.n_iterations,
            "ILU(0) {} iters vs none {} iters",
            ilu.n_iterations,
            none.n_iterations
        );
    }

    #[test]
    fn warm_start_from_the_answer_costs_no_iterations() {
        let n = 30;
        let a = asym_tridiag(n, 3.0, -1.0, -0.5);
        let x_true: Vec<f64> = (0..n).map(|i| 1.0 + i as f64).collect();
        let b = a.multiply(&x_true);
        let settings = SolverSettings {
            tolerance: 1e-10,
            max_iter: 500,
        };
        let (_, perf) = krylov_solve(
            &a,
            &b,
            Some(&x_true),
            KrylovMethod::BiCGStab,
            KrylovOptions::default(),
            &settings,
        );
        assert!(perf.converged);
        assert_eq!(perf.n_iterations, 0, "warm start should exit immediately");
    }
}
