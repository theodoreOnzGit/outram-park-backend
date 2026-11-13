// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PORTED, read 2026-11-01, from:
//
// 1. This workspace's `crates/outram-park-fork-pflotran/src/solver/mod.rs`,
//    commit 69f3f242a483b44335a35a5584538dd7df64e035: the Newton driver
//    `NewtonSolver::solve` (residual, convergence test, linear solve
//    `J dx = -F`, non-converged linear solve tolerated, non-finite step
//    fatal) and its backtracking Armijo `line_search` (c = 1e-4, halving,
//    best-finite fallback). GPL-3.0 (OUTRAM PARK contributors).
//
// 2. PETSc, commit e413143d55b0811f9b914603d9694184c0392685, from
//    https://github.com/petsc/petsc, `src/snes/interface/snesut.c`,
//    `SNESConvergedDefault` (lines 720-760): the absolute, relative and
//    step-length tests. Copyright (c) 1991-2025, UChicago Argonne, LLC and
//    the PETSc Developers and Contributors; BSD-2-Clause (full notice in
//    `mffd.rs` and NOTICE).
//
// Distributed under GPL-3.0-only.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.

//! Jacobian-free Newton–Krylov (JFNK) for `F(x) = 0` (gh:#1066, RELAP-7
//! Theory Manual §5.5, cited only).
//!
//! # What is here
//!
//! - [`jfnk`] — Newton with each step `J dx = -F` solved by restarted GMRES
//!   on the finite-difference product [`MffdOperator`]; no Jacobian is ever
//!   formed.
//! - [`jfnk_preconditioned`] — the same, right-preconditioned by the LU
//!   factors of a caller-assembled **approximate** dense Jacobian
//!   ([`DenseLuPreconditioner`]), rebuilt at every Newton iterate.
//! - [`gmres`], [`LinearOperator`], [`OperatorPreconditioner`],
//!   [`IdentityPreconditioner`], [`KrylovSettings`], [`KrylovResult`] —
//!   ported from `outram_foam_basic_lib::linear_operator` (see [`gmres`'s
//!   module](self::gmres)).
//! - [`MffdOperator`] — PETSc's `MatMult_MFFD` with the Walker–Pernice
//!   differencing parameter (see [`mffd`'s module](self::mffd)).
//!
//! # Lineage
//!
//! The driver **generalises `outram-park-fork-pflotran`'s Newton–Krylov
//! driver** (`src/solver/mod.rs`, which uses an assembled LDU Jacobian),
//! as gh:#1066 asks: the loop, the tolerated inexact linear solve, the
//! non-finite checks and the Armijo backtracking line search are its; the
//! Jacobian becomes the matrix-free [`MffdOperator`], the preconditioner a
//! dense LU of the caller's approximate Jacobian, and the convergence test is
//! PETSc's `SNESConvergedDefault` so that it has the three tests the issue
//! names:
//!
//! ```text
//! it == 0:  ttol = rtol * ||F_0||
//! converged (absolute)  if ||F|| <  abs_tol            (also at it == 0)
//! converged (relative)  if ||F|| <= ttol               (it > 0)
//! converged (step)      if ||s|| <  step_tol * ||x||   (it > 0)
//! ```
//!
//! with `s` the step actually taken (`lambda dx`) and `x` the new iterate.
//! (pflotran's driver had only the first two, with a strict `<` on the
//! relative test; PETSc's `<=` is kept.)
//!
//! # Deviations, each stated
//!
//! - **DEVIATION — line search.** PETSc's `SNESNEWTONLS` defaults to a cubic
//!   backtracking (`bt`) line search; this keeps pflotran's halving Armijo
//!   search, which is the workspace code being generalised.
//! - **DEVIATION — constant forcing term.** Each GMRES solve uses the fixed
//!   relative tolerance `linear.tolerance` (pflotran's behaviour); no
//!   Eisenstat–Walker adaptive forcing.
//! - **DEVIATION — PETSc defaults not adopted.** `JfnkConfig::default()`
//!   uses pflotran's `abs_tol = 1e-9`, `rel_tol = 1e-8`, 50 iterations and 10
//!   backtracks, with PETSc's `stol = 1e-8`, and a forcing term of `1e-4`
//!   instead of pflotran's `1e-8` linear tolerance (measured reason on
//!   [`JfnkConfig::default`]).
//! - Not ported: divergence tolerance (`divtol`), function-evaluation cap,
//!   `forceiteration`.
//!
//! # Units
//!
//! Bare `f64`, per the crate's units policy.

pub mod gmres;
pub mod mffd;

pub use gmres::{
    gmres, IdentityPreconditioner, KrylovResult, KrylovSettings, LinearOperator,
    OperatorPreconditioner,
};
pub use mffd::{MffdOperator, PETSC_SQRT_MACHINE_EPSILON};

use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;

use crate::linalg::{lu_decomp, lu_svx, nrm2, LuPermutation, Matrix};
use crate::{PetirError, Result};

/// Right preconditioner `M^{-1} = (P L U)^{-1}` from GSL's LU of a dense
/// approximate Jacobian.
#[derive(Debug, Clone, PartialEq)]
pub struct DenseLuPreconditioner {
    lu: Matrix,
    p: LuPermutation,
}

impl DenseLuPreconditioner {
    /// Factor the square matrix `approx_jacobian` ([`crate::linalg::lu_decomp`]).
    ///
    /// # Errors
    ///
    /// [`PetirError::Invalid`] if not square; [`PetirError::Singular`] if a
    /// pivot is exactly zero.
    pub fn new(mut approx_jacobian: Matrix) -> Result<Self> {
        let p = lu_decomp(&mut approx_jacobian)?;
        if let Some(col) = p.zero_pivot {
            return Err(PetirError::Singular { col });
        }
        Ok(Self {
            lu: approx_jacobian,
            p,
        })
    }
}

impl OperatorPreconditioner for DenseLuPreconditioner {
    /// `z = M^{-1} r`. Cannot fail after a successful [`new`](Self::new)
    /// with matching lengths; on a length mismatch `z` is left equal to `r`
    /// (no preconditioning) rather than panicking.
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        z.copy_from_slice(r);
        if lu_svx(&self.lu, &self.p, z).is_err() {
            z.copy_from_slice(r);
        }
    }
}

/// The two preconditioners the driver can use, dispatched by enum.
enum DriverPreconditioner {
    Identity(IdentityPreconditioner),
    DenseLu(DenseLuPreconditioner),
}

impl OperatorPreconditioner for DriverPreconditioner {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        match self {
            Self::Identity(p) => p.apply(r, z),
            Self::DenseLu(p) => p.apply(r, z),
        }
    }
}

/// JFNK configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JfnkConfig {
    /// Newton iterations before giving up ([`PetirError::MaxIterations`]).
    pub max_iterations: usize,
    /// Absolute test: converged when `||F||_2 < abs_tol`.
    pub abs_tol: f64,
    /// Relative test: converged when `||F||_2 <= rel_tol * ||F_0||_2`.
    pub rel_tol: f64,
    /// Step test: converged when `||lambda dx||_2 < step_tol * ||x||_2`.
    pub step_tol: f64,
    /// The inner GMRES settings; `linear.tolerance` is the (constant)
    /// forcing term.
    pub linear: KrylovSettings,
    /// Line-search halvings per step; `0` is full-step Newton.
    pub max_backtracks: usize,
    /// `error_rel` of the finite-difference product
    /// ([`PETSC_SQRT_MACHINE_EPSILON`] by default).
    pub error_rel: f64,
}

impl Default for JfnkConfig {
    /// 50 iterations, `abs_tol = 1e-9`, `rel_tol = 1e-8` (pflotran),
    /// `step_tol = 1e-8` (PETSc), GMRES(30) with at most 1000 iterations and
    /// forcing term `linear.tolerance = 1e-4`, 10 backtracks,
    /// `error_rel = PETSC_SQRT_MACHINE_EPSILON`.
    ///
    /// **Why the forcing term is `1e-4`, not [`KrylovSettings`]' `1e-8`:**
    /// measured 2026-11-01 (`tests/jfnk_bratu_and_nonlinear_heat.rs`), at
    /// `1e-8` every GMRES solve after the first Newton step ran to its
    /// iteration cap on both test problems, because the finite-difference
    /// product carries noise of order `eps |F terms| / h_fd`, above `1e-8 ||F||`
    /// once `||F||` is small. The Newton answer was unchanged and the work
    /// was 50–100x larger. `1e-4` is inside Knoll & Keyes' (2004) range for a
    /// constant forcing term.
    fn default() -> Self {
        Self {
            max_iterations: 50,
            abs_tol: 1e-9,
            rel_tol: 1e-8,
            step_tol: 1e-8,
            linear: KrylovSettings {
                tolerance: 1e-4,
                ..KrylovSettings::default()
            },
            max_backtracks: 10,
            error_rel: PETSC_SQRT_MACHINE_EPSILON,
        }
    }
}

/// Which stopping test ended a converged solve (PETSc's
/// `SNES_CONVERGED_FNORM_ABS`, `_FNORM_RELATIVE`, `_SNORM_RELATIVE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JfnkConverged {
    /// `||F|| < abs_tol`.
    AbsoluteResidual,
    /// `||F|| <= rel_tol ||F_0||`.
    RelativeResidual,
    /// `||lambda dx|| < step_tol ||x||`.
    StepLength,
}

/// Outcome of a converged JFNK solve.
#[derive(Debug, Clone, PartialEq)]
pub struct JfnkReport {
    /// Newton iterations (steps taken).
    pub iterations: usize,
    /// Which test was met.
    pub reason: JfnkConverged,
    /// `||F||_2` at exit.
    pub final_residual_norm: f64,
    /// `||F||_2` at each iterate, starting with `||F_0||`.
    pub residual_history: Vec<f64>,
    /// GMRES inner iterations of each Newton step.
    pub linear_iterations: Vec<usize>,
    /// Total evaluations of `F`, including the finite-difference products,
    /// GMRES's true-residual checks and the line search.
    pub function_evaluations: usize,
}

impl JfnkReport {
    /// Sum of [`linear_iterations`](Self::linear_iterations).
    pub fn total_linear_iterations(&self) -> usize {
        self.linear_iterations.iter().sum()
    }
}

/// Solve `F(x) = 0` by Jacobian-free Newton–Krylov, unpreconditioned.
///
/// `f(x, out)` writes the residual; `x` holds the initial guess and receives
/// the root. See the [module docs](self) for the algorithm and stopping tests.
///
/// # Errors
///
/// - [`PetirError::Invalid`] for an empty `x`.
/// - [`PetirError::Domain`] for a non-finite residual or Newton step.
/// - [`PetirError::MaxIterations`] if no test is met in
///   `config.max_iterations` steps (`x` holds the last iterate).
///
/// # Example
///
/// ```
/// use petir::newton_krylov::{jfnk, JfnkConfig};
/// // x0^2 - 4 = 0, x0 x1 - 2 = 0  ->  (2, 1)
/// let f = |x: &[f64], r: &mut [f64]| { r[0] = x[0] * x[0] - 4.0; r[1] = x[0] * x[1] - 2.0; };
/// let mut x = vec![1.0, 1.0];
/// let rep = jfnk(&f, &mut x, &JfnkConfig::default()).unwrap();
/// assert!((x[0] - 2.0).abs() < 1e-7 && (x[1] - 1.0).abs() < 1e-7);
/// assert!(rep.iterations < 10);
/// ```
pub fn jfnk<F>(f: &F, x: &mut [f64], config: &JfnkConfig) -> Result<JfnkReport>
where
    F: Fn(&[f64], &mut [f64]),
{
    let none: Option<&fn(&[f64], &mut Matrix)> = None;
    jfnk_impl(f, none, x, config)
}

/// Solve `F(x) = 0` by JFNK, right-preconditioned at every Newton iterate by
/// the LU of `approx_jacobian(x, J)`, which writes an approximation of
/// `dF/dx` into the `n x n` `J` (zeroed before each call).
///
/// The approximation only has to make `J M^{-1}` well conditioned: a
/// Picard (frozen-coefficient) matrix, a lagged Jacobian, the linear part
/// alone. The Newton step is still the exact-residual JFNK step.
///
/// # Errors
///
/// As [`jfnk`], plus [`PetirError::Singular`] if the approximate Jacobian is
/// singular at an iterate.
pub fn jfnk_preconditioned<F, P>(
    f: &F,
    approx_jacobian: &P,
    x: &mut [f64],
    config: &JfnkConfig,
) -> Result<JfnkReport>
where
    F: Fn(&[f64], &mut [f64]),
    P: Fn(&[f64], &mut Matrix),
{
    jfnk_impl(f, Some(approx_jacobian), x, config)
}

fn jfnk_impl<F, P>(
    f: &F,
    approx_jacobian: Option<&P>,
    x: &mut [f64],
    config: &JfnkConfig,
) -> Result<JfnkReport>
where
    F: Fn(&[f64], &mut [f64]),
    P: Fn(&[f64], &mut Matrix),
{
    let n = x.len();
    if n == 0 {
        return Err(PetirError::Invalid);
    }

    let mut fv = vec![0.0_f64; n];
    let mut rhs = vec![0.0_f64; n];
    let mut x_trial = vec![0.0_f64; n];
    let mut f_trial = vec![0.0_f64; n];

    let mut history: Vec<f64> = Vec::with_capacity(config.max_iterations + 1);
    let mut linear_iterations: Vec<usize> = Vec::with_capacity(config.max_iterations);
    let mut evals = 0usize;
    let mut ttol = 0.0_f64;
    let mut snorm = 0.0_f64;
    let mut xnorm = 0.0_f64;

    for iter in 0..=config.max_iterations {
        f(x, &mut fv);
        evals += 1;
        let fnorm = nrm2(&fv);
        if !fnorm.is_finite() {
            return Err(PetirError::Domain);
        }
        history.push(fnorm);

        // SNESConvergedDefault (snesut.c:720-760).
        if iter == 0 {
            ttol = fnorm * config.rel_tol;
        }
        let mut reason = None;
        if fnorm < config.abs_tol {
            reason = Some(JfnkConverged::AbsoluteResidual);
        }
        if iter > 0 && reason.is_none() {
            if fnorm <= ttol {
                reason = Some(JfnkConverged::RelativeResidual);
            } else if snorm < config.step_tol * xnorm {
                reason = Some(JfnkConverged::StepLength);
            }
        }
        if let Some(reason) = reason {
            return Ok(JfnkReport {
                iterations: iter,
                reason,
                final_residual_norm: fnorm,
                residual_history: history,
                linear_iterations,
                function_evaluations: evals,
            });
        }

        if iter == config.max_iterations {
            break;
        }

        // Preconditioner from the approximate Jacobian at x.
        let precond = match approx_jacobian {
            None => DriverPreconditioner::Identity(IdentityPreconditioner),
            Some(pj) => {
                let mut j = Matrix::zeros(n, n)?;
                pj(x, &mut j);
                if j.as_slice().iter().any(|v| !v.is_finite()) {
                    return Err(PetirError::Domain);
                }
                DriverPreconditioner::DenseLu(DenseLuPreconditioner::new(j)?)
            }
        };

        // J dx = -F with J the finite-difference product at (x, F(x)).
        for (r, &fi) in rhs.iter_mut().zip(fv.iter()) {
            *r = -fi;
        }
        let op = MffdOperator::new(f, x.to_vec(), fv.clone(), config.error_rel);
        let (dx, lin) = gmres(&op, &rhs, None, &precond, &config.linear);
        evals += op.function_evaluations();
        linear_iterations.push(lin.n_iterations);
        // A non-converged inner solve is tolerated (inexact Newton); only a
        // non-finite direction is fatal.
        if dx.iter().any(|v| !v.is_finite()) {
            return Err(PetirError::Domain);
        }

        let (lambda, used) =
            line_search(f, x, &dx, fnorm, config.max_backtracks, &mut x_trial, &mut f_trial)?;
        evals += used;

        for (xi, &di) in x.iter_mut().zip(dx.iter()) {
            *xi += lambda * di;
        }
        let step: Vec<f64> = dx.iter().map(|&d| lambda * d).collect();
        snorm = nrm2(&step);
        xnorm = nrm2(x);
    }

    Err(PetirError::MaxIterations)
}

/// pflotran's backtracking Armijo line search (`solver/mod.rs:338-381`):
/// `lambda = 1, 1/2, ...`, accept the first with
/// `||F(x + lambda dx)|| <= (1 - 1e-4 lambda) ||F(x)||`, else the `lambda`
/// with the smallest finite trial norm. Returns `(lambda, evaluations)`.
fn line_search<F>(
    f: &F,
    x: &[f64],
    dx: &[f64],
    base_norm: f64,
    max_backtracks: usize,
    x_trial: &mut [f64],
    f_trial: &mut [f64],
) -> Result<(f64, usize)>
where
    F: Fn(&[f64], &mut [f64]),
{
    const ARMIJO_C: f64 = 1e-4;
    let mut lambda = 1.0_f64;
    let mut best_lambda = 1.0_f64;
    let mut best_norm = f64::INFINITY;
    let mut used = 0usize;

    for _ in 0..=max_backtracks {
        for (xt, (&xi, &di)) in x_trial.iter_mut().zip(x.iter().zip(dx.iter())) {
            *xt = xi + lambda * di;
        }
        f(x_trial, f_trial);
        used += 1;
        let trial_norm = nrm2(f_trial);

        if trial_norm.is_finite() && trial_norm < best_norm {
            best_norm = trial_norm;
            best_lambda = lambda;
        }
        if trial_norm.is_finite() && trial_norm <= (1.0 - ARMIJO_C * lambda) * base_norm {
            return Ok((lambda, used));
        }
        lambda *= 0.5;
    }

    if best_norm.is_finite() {
        Ok((best_lambda, used))
    } else {
        Err(PetirError::Domain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quadratic(x: &[f64], r: &mut [f64]) {
        r[0] = x[0] * x[0] - 4.0;
        r[1] = x[0] * x[1] - 2.0;
    }

    #[test]
    fn preconditioned_with_the_exact_jacobian_needs_one_gmres_iteration() {
        let jac = |x: &[f64], j: &mut Matrix| {
            j.set(0, 0, 2.0 * x[0]);
            j.set(1, 0, x[1]);
            j.set(1, 1, x[0]);
        };
        let mut x = vec![1.0, 1.0];
        let rep = jfnk_preconditioned(&quadratic, &jac, &mut x, &JfnkConfig::default()).unwrap();
        assert!((x[0] - 2.0).abs() < 1e-8 && (x[1] - 1.0).abs() < 1e-8);
        // J M^{-1} = I up to the finite-difference error.
        assert!(rep.linear_iterations.iter().all(|&k| k <= 2), "{rep:?}");
    }

    #[test]
    fn step_length_test_can_end_the_solve() {
        let cfg = JfnkConfig {
            abs_tol: 0.0,
            rel_tol: 0.0,
            step_tol: 1e-6,
            ..JfnkConfig::default()
        };
        let mut x = vec![1.0, 1.0];
        let rep = jfnk(&quadratic, &mut x, &cfg).unwrap();
        assert_eq!(rep.reason, JfnkConverged::StepLength);
    }

    #[test]
    fn absolute_test_applies_at_iteration_zero() {
        let mut x = vec![2.0, 1.0];
        let rep = jfnk(&quadratic, &mut x, &JfnkConfig::default()).unwrap();
        assert_eq!(rep.iterations, 0);
        assert_eq!(rep.reason, JfnkConverged::AbsoluteResidual);
    }

    #[test]
    fn exhausted_budget_and_bad_input_are_errors() {
        let cfg = JfnkConfig {
            max_iterations: 1,
            abs_tol: 0.0,
            rel_tol: 0.0,
            step_tol: 0.0,
            ..JfnkConfig::default()
        };
        let mut x = vec![1.0, 1.0];
        assert_eq!(jfnk(&quadratic, &mut x, &cfg), Err(PetirError::MaxIterations));
        let mut empty: Vec<f64> = Vec::new();
        assert_eq!(
            jfnk(&quadratic, &mut empty, &JfnkConfig::default()),
            Err(PetirError::Invalid)
        );
        let singular = |_x: &[f64], _j: &mut Matrix| {};
        let mut x = vec![1.0, 1.0];
        assert!(matches!(
            jfnk_preconditioned(&quadratic, &singular, &mut x, &JfnkConfig::default()),
            Err(PetirError::Singular { .. })
        ));
    }

    #[test]
    fn zero_direction_gives_zero_product() {
        let mut fu = vec![0.0; 2];
        quadratic(&[1.0, 1.0], &mut fu);
        let op = MffdOperator::new(quadratic, vec![1.0, 1.0], fu, PETSC_SQRT_MACHINE_EPSILON);
        let mut y = [9.0; 2];
        op.apply(&[0.0, 0.0], &mut y);
        assert_eq!(y, [0.0, 0.0]);
        assert_eq!(op.function_evaluations(), 0);
        assert_eq!(op.differencing_parameter(&[0.0, 0.0]), None);
    }
}
