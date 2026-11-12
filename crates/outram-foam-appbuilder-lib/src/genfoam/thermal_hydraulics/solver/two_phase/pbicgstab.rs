// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! `PBiCGStab` with the `diagonal` preconditioner, translated operation for
//! operation from OpenFOAM-v2506 (gh:#1361 round 3).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! # Upstream
//!
//! - `src/OpenFOAM/matrices/lduMatrix/solvers/PBiCGStab/PBiCGStab.C`,
//!   `PBiCGStab::scalarSolve` (the whole function);
//! - `.../preconditioners/diagonalPreconditioner/diagonalPreconditioner.C`
//!   (`rD = 1/diag`, `wA = rD*rA`);
//! - `src/OpenFOAM/matrices/LduMatrix/LduMatrix/SolverPerformance.C`
//!   (`checkConvergence`, `checkSingularity`; `small_ = 1e-20`,
//!   `vsmall_ = VSMALL`);
//! - `lduMatrix::solver::normFactor` (`lduMatrixSolver.C:174-193`).
//!
//! # Why this exists
//!
//! GeN-Foam's `alpha.<phase>` equation (`solveAlphaImplicitUpwind.H`) is
//! solved with whatever the case's `solvers` block selects for
//! `alpha.vapour`, which for `1D_boiling` is the catch-all `".*"`
//! `PBiCGStab`/`diagonal`. That solve stops at a tolerance **normalised by the
//! matrix norm**, so the tail of a decaying vapour front (cells at `1e-9` and
//! below) is solver noise in upstream, not the exact solution of the
//! triangular upwind system. The port solved it with ILU(0)-BiCGStab to 1e-13,
//! i.e. exactly, and the two differ by a fraction of a percent in the tail
//! cells (gh:#1361 round 3, boiling onset): the first divergence of the port
//! from the real binary on `1D_boiling`. Running the same algorithm with the
//! same stopping rule reproduces that noise.
//!
//! The sums are accumulated in cell order, as `gSumProd` and friends do for a
//! serial run, and the matrix-vector product is
//! `LduMatrix::multiply`.

use outram_foam_basic_lib::prelude::LduMatrix;

/// `SolverPerformance::small_`.
const SMALL: f64 = 1e-20;
/// `VSMALL` for doubles.
const VSMALL: f64 = 1e-300;
/// `lduMatrix::defaultMaxIter`.
pub(crate) const DEFAULT_MAX_ITER: usize = 1000;

/// Result of a solve, as OpenFOAM's `solverPerformance`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SolvePerformance {
    /// Iterations performed.
    pub iterations: usize,
    /// Normalised residual before the first iteration.
    pub initial_residual: f64,
    /// Normalised residual at exit.
    pub final_residual: f64,
}

fn g_sum_mag(v: &[f64]) -> f64 {
    let mut s = 0.0;
    for x in v {
        s += x.abs();
    }
    s
}

fn g_sum_prod(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for i in 0..a.len() {
        s += a[i] * b[i];
    }
    s
}

/// `solverPerformance::checkConvergence`.
fn converged(final_residual: f64, initial_residual: f64, tolerance: f64, rel_tol: f64) -> bool {
    final_residual < tolerance || (rel_tol > SMALL && final_residual < rel_tol * initial_residual)
}

/// `lduMatrix::solver::normFactor(psi, source, Apsi, tmpField)`.
fn norm_factor(a: &LduMatrix, psi: &[f64], source: &[f64], apsi: &[f64]) -> f64 {
    let n = psi.len();
    // sumA
    let mut sum_a = a.diag.clone();
    for f in 0..a.n_internal_faces {
        sum_a[a.neighbour[f]] += a.lower[f];
        sum_a[a.owner[f]] += a.upper[f];
    }
    let mut avg = 0.0;
    for x in psi {
        avg += x;
    }
    let avg = avg / n.max(1) as f64;
    let mut s = 0.0;
    for c in 0..n {
        let t = sum_a[c] * avg;
        s += (apsi[c] - t).abs() + (source[c] - t).abs();
    }
    s + SMALL
}

/// `PBiCGStab::scalarSolve` with `preconditioner diagonal`. `psi` is the
/// initial guess and receives the solution.
///
/// `tolerance`, `rel_tol`, `max_iter` (default [`DEFAULT_MAX_ITER`]) and
/// `min_iter` (default 0) are the entries of the case's `solvers` dictionary.
pub(crate) fn pbicgstab_diagonal(
    a: &LduMatrix,
    source: &[f64],
    psi: &mut [f64],
    tolerance: f64,
    rel_tol: f64,
    max_iter: usize,
    min_iter: usize,
) -> SolvePerformance {
    let n = psi.len();
    let ya0 = a.multiply(psi);
    let mut r_a: Vec<f64> = (0..n).map(|c| source[c] - ya0[c]).collect();
    let norm = norm_factor(a, psi, source, &ya0);
    let initial = g_sum_mag(&r_a) / norm;
    let mut perf = SolvePerformance { iterations: 0, initial_residual: initial, final_residual: initial };
    if min_iter == 0 && converged(perf.final_residual, initial, tolerance, rel_tol) {
        return perf;
    }

    let r_d: Vec<f64> = a.diag.iter().map(|d| 1.0 / d).collect();
    let r_a0 = r_a.clone();
    let mut p_a = vec![0.0; n];
    let mut y_a = vec![0.0; n];
    let mut ay_a = vec![0.0; n];
    let mut s_a = vec![0.0; n];
    let mut z_a = vec![0.0; n];
    let mut r_a0r_a = 0.0_f64;
    let mut alpha = 0.0_f64;
    let mut omega = 0.0_f64;

    loop {
        let r_a0r_a_old = r_a0r_a;
        r_a0r_a = g_sum_prod(&r_a0, &r_a);
        if r_a0r_a.abs() < VSMALL {
            break;
        }
        if perf.iterations == 0 {
            p_a.copy_from_slice(&r_a);
        } else {
            if omega.abs() < VSMALL {
                break;
            }
            let beta = (r_a0r_a / r_a0r_a_old) * (alpha / omega);
            for c in 0..n {
                p_a[c] = r_a[c] + beta * (p_a[c] - omega * ay_a[c]);
            }
        }
        for c in 0..n {
            y_a[c] = r_d[c] * p_a[c];
        }
        ay_a = a.multiply(&y_a);
        let r_a0ay_a = g_sum_prod(&r_a0, &ay_a);
        alpha = r_a0r_a / r_a0ay_a;
        for c in 0..n {
            s_a[c] = r_a[c] - alpha * ay_a[c];
        }
        perf.final_residual = g_sum_mag(&s_a) / norm;
        if perf.iterations >= min_iter && converged(perf.final_residual, initial, tolerance, rel_tol) {
            for c in 0..n {
                psi[c] += alpha * y_a[c];
            }
            perf.iterations += 1;
            return perf;
        }
        for c in 0..n {
            z_a[c] = r_d[c] * s_a[c];
        }
        let t_a = a.multiply(&z_a);
        let t_at_a = g_sum_prod(&t_a, &t_a);
        omega = g_sum_prod(&t_a, &s_a) / t_at_a;
        for c in 0..n {
            psi[c] += alpha * y_a[c] + omega * z_a[c];
            r_a[c] = s_a[c] - omega * t_a[c];
        }
        perf.final_residual = g_sum_mag(&r_a) / norm;
        perf.iterations += 1;
        let go_on = (perf.iterations < max_iter && !converged(perf.final_residual, initial, tolerance, rel_tol))
            || perf.iterations < min_iter;
        if !go_on {
            break;
        }
    }
    perf
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1D convection-diffusion-like non-symmetric tridiagonal system with a
    /// decaying exact solution `0.5^i` (a vapour-front tail).
    fn upwind_chain(n: usize) -> (LduMatrix, Vec<f64>, Vec<f64>) {
        let nf = n - 1;
        let owner: Vec<usize> = (0..nf).collect();
        let neighbour: Vec<usize> = (1..n).collect();
        let mut a = LduMatrix::new(n, owner, neighbour);
        a.diag = vec![3.0; n];
        a.lower = vec![-1.2; nf];
        a.upper = vec![-0.7; nf];
        let x_exact: Vec<f64> = (0..n).map(|i| 0.5_f64.powi(i as i32)).collect();
        let b = a.multiply(&x_exact);
        (a, b, x_exact)
    }

    /// Run to a tight tolerance the solver reaches the exact solution; at a loose
    /// tolerance the small tail is NOT resolved (the property the port relies on).
    #[test]
    fn solves_a_triangular_chain_and_leaves_the_tail_at_a_loose_tolerance() {
        let (a, b, x_exact) = upwind_chain(40);
        let mut x = vec![0.0; 40];
        let tight = pbicgstab_diagonal(&a, &b, &mut x, 1e-12, 0.0, DEFAULT_MAX_ITER, 0);
        assert!(tight.final_residual < 1e-12);
        for i in 0..40 {
            assert!((x[i] - x_exact[i]).abs() <= 1e-9 * x_exact[0], "cell {i}");
        }
        let mut y = vec![0.0; 40];
        let loose = pbicgstab_diagonal(&a, &b, &mut y, 1e-3, 0.0, DEFAULT_MAX_ITER, 0);
        assert!(loose.iterations >= 1 && loose.iterations < tight.iterations);
        let tail_err = (y[39] - x_exact[39]).abs() / x_exact[39];
        assert!(tail_err > 1e-3, "the unresolved tail should differ from exact, got {tail_err}");
    }

    /// An already-converged guess returns with zero iterations and is untouched.
    #[test]
    fn a_converged_guess_is_left_alone() {
        let (a, b, x_exact) = upwind_chain(10);
        let mut x = x_exact.clone();
        let perf = pbicgstab_diagonal(&a, &b, &mut x, 1e-10, 0.0, DEFAULT_MAX_ITER, 0);
        assert_eq!(perf.iterations, 0);
        assert_eq!(x, x_exact);
    }

    /// `minIter` forces iterations although the tolerance is met earlier.
    #[test]
    fn min_iter_forces_iterations() {
        let (a, b, _) = upwind_chain(40);
        let mut x = vec![0.0; 40];
        let free = pbicgstab_diagonal(&a, &b, &mut x, 1e-3, 0.0, DEFAULT_MAX_ITER, 0);
        let mut y = vec![0.0; 40];
        let forced = pbicgstab_diagonal(&a, &b, &mut y, 1e-3, 0.0, DEFAULT_MAX_ITER, free.iterations + 2);
        assert!(forced.iterations >= free.iterations + 2);
    }
}
