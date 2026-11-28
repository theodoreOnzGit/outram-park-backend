// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! OpenFOAM-v2506 `smoothSolver` with the `symGaussSeidel` smoother, every control honoured: `tolerance`, `relTol`, `maxIter`, `minIter`
//! and `nSweeps` (also negative: a fixed number of sweeps with no residual) (gh:#1942, README "Round 79", Part C).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! # Upstream (ESI v2506, `src/OpenFOAM/matrices/lduMatrix/`)
//!
//! | piece | upstream | here |
//! |---|---|---|
//! | `nSweeps` default 1 | `solvers/smoothSolver/smoothSolver.C:73-77` (`readControls`) | [`SmoothSolverControls::default`] |
//! | `nSweeps < 0`: `-nSweeps` sweeps, `nIterations -= nSweeps`, no residual | `smoothSolver.C:93-117` | [`smooth_solver_sym_gauss_seidel`] |
//! | `Amul`, `normFactor`, initial residual `gSumMag(source - Apsi)/normFactor` | `smoothSolver.C:119-146`, `lduMatrixSolver.C` (`normFactor`) | `a.multiply`, `pcg::norm_factor`, `pcg::g_sum_mag` |
//! | `if (minIter_ > 0 \|\| !checkConvergence(tolerance, relTol))` then `do { smooth(nSweeps); residual } while (((nIter += nSweeps) < maxIter && !converged) \|\| nIter < minIter)` | `smoothSolver.C:155-209`, `SolverPerformance.C:63-88` | the loop below, `pcg::converged` |
//! | the smoother | `smoothers/symGaussSeidel/symGaussSeidelSmoother.C:74-199` (no interfaces) | `gamg::sym_gauss_seidel_smooth` (round 72, reused) |
//! | `matrix_.residual` | `lduMatrixATmul.C` (`source - diag psi`, then faces) | `gamg::ldu_residual` (reused) |
//!
//! The earlier `nSweeps 1`, `relTol 0` translation (`one_phase::smooth_solve_sym_gauss_seidel_openfoam`, round 46) is the special case
//! `n_sweeps = 1`, `rel_tol = 0`: the two are bit-identical there (pinned by a test below), and that function stays as it is (other
//! solvers call it).
//!
//! Not ported: coupled interfaces (`initMatrixInterfaces` / `updateMatrixInterfaces`; the callers' matrices have none), other smoothers.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::gamg::{ldu_residual, sym_gauss_seidel_smooth};
use super::pcg::{converged, g_sum_mag, norm_factor};
use outram_foam_basic_lib::prelude::LduMatrix;

/// The controls of a `smoothSolver` entry (`lduMatrix::solver::readControls` defaults and `nSweeps 1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmoothSolverSweeps {
    /// `nSweeps` (default 1; negative: a fixed number `-nSweeps` of sweeps, no residual evaluated).
    pub n_sweeps: i64,
    /// `maxIter` (default 1000).
    pub max_iter: usize,
}

impl Default for SmoothSolverSweeps {
    fn default() -> Self {
        Self { n_sweeps: 1, max_iter: 1000 }
    }
}

/// All the controls of a `smoothSolver` solve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothSolverControls {
    /// `tolerance` (default 1e-6).
    pub tolerance: f64,
    /// `relTol` (default 0).
    pub rel_tol: f64,
    /// `minIter` (default 0).
    pub min_iter: usize,
    /// `nSweeps` and `maxIter`.
    pub sweeps: SmoothSolverSweeps,
}

impl Default for SmoothSolverControls {
    fn default() -> Self {
        Self { tolerance: 1e-6, rel_tol: 0.0, min_iter: 0, sweeps: SmoothSolverSweeps::default() }
    }
}

/// `solverPerformance` of a `smoothSolver` solve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothSolverPerformance {
    /// `nIterations` (counted in sweeps: `+= nSweeps` per smoothing call).
    pub iterations: usize,
    /// Initial residual (0 for a fixed-sweep `nSweeps < 0` solve, which evaluates none).
    pub initial_residual: f64,
    /// Final residual.
    pub final_residual: f64,
}

/// `smoothSolver::solve` with the `symGaussSeidel` smoother on `a psi = source`; `psi` is the initial guess and receives the solution.
#[must_use]
pub fn smooth_solver_sym_gauss_seidel(a: &LduMatrix, source: &[f64], psi: &mut [f64], ctl: &SmoothSolverControls) -> SmoothSolverPerformance {
    let n_sweeps = ctl.sweeps.n_sweeps;
    if n_sweeps < 0 {
        // fixed number of sweeps: `solverPerf.nIterations() -= nSweeps_`, residuals left at 0
        let k = n_sweeps.unsigned_abs() as usize;
        sym_gauss_seidel_smooth(a, psi, source, k);
        return SmoothSolverPerformance { iterations: k, initial_residual: 0.0, final_residual: 0.0 };
    }
    let apsi = a.multiply(psi);
    let norm = norm_factor(a, psi, source, &apsi);
    let r0: Vec<f64> = (0..psi.len()).map(|c| source[c] - apsi[c]).collect();
    let initial = g_sum_mag(&r0) / norm;
    let mut perf = SmoothSolverPerformance { iterations: 0, initial_residual: initial, final_residual: initial };
    if ctl.min_iter > 0 || !converged(perf.final_residual, initial, ctl.tolerance, ctl.rel_tol) {
        let step = n_sweeps as usize;
        loop {
            sym_gauss_seidel_smooth(a, psi, source, step);
            let r = ldu_residual(a, psi, source);
            perf.final_residual = g_sum_mag(&r) / norm;
            perf.iterations += step;
            let go_on = (perf.iterations < ctl.sweeps.max_iter && !converged(perf.final_residual, initial, ctl.tolerance, ctl.rel_tol))
                || perf.iterations < ctl.min_iter;
            if !go_on {
                break;
            }
        }
    }
    perf
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::prelude::SolverSettings;

    /// An asymmetric 2-D grid matrix (upwind-like), diagonally dominant, with upper-triangular face order.
    fn grid(nx: usize, ny: usize) -> (LduMatrix, Vec<f64>) {
        let n = nx * ny;
        let (mut owner, mut neighbour) = (Vec::new(), Vec::new());
        for c in 0..n {
            let (i, j) = (c % nx, c / nx);
            if i + 1 < nx {
                owner.push(c);
                neighbour.push(c + 1);
            }
            if j + 1 < ny {
                owner.push(c);
                neighbour.push(c + nx);
            }
        }
        let nf = owner.len();
        let mut a = LduMatrix::new(n, owner, neighbour);
        a.upper = (0..nf).map(|f| -1.0 - 0.01 * (f % 7) as f64).collect();
        a.lower = (0..nf).map(|f| -0.6 - 0.02 * (f % 5) as f64).collect();
        a.diag = (0..n).map(|c| 5.0 + 0.1 * (c % 3) as f64).collect();
        let x: Vec<f64> = (0..n).map(|c| 1.0 + (c as f64).sin()).collect();
        let b = a.multiply(&x);
        (a, b)
    }

    /// `nSweeps 1`, `relTol 0`, `maxIter 1000` is bit-identical to the round 46 translation (residuals, sweeps, solution), at a tolerance it
    /// reaches and at one it reaches only by `maxIter`, and with `minIter`.
    #[test]
    fn n_sweeps_one_rel_tol_zero_is_bit_identical_to_the_round_46_translation() {
        let (a, b) = grid(9, 7);
        for (tol, min_iter) in [(1e-8, 0usize), (1e-300, 0), (1e-3, 5), (1e30, 0)] {
            let mut x_old = vec![0.0; a.n_cells];
            let (sw, ini, fin) = crate::genfoam::thermal_hydraulics::solver::one_phase::smooth_solve_sym_gauss_seidel_openfoam(
                &a,
                &b,
                &mut x_old,
                SolverSettings { tolerance: tol, max_iter: 1000 },
                min_iter,
            );
            let mut x_new = vec![0.0; a.n_cells];
            let ctl = SmoothSolverControls { tolerance: tol, rel_tol: 0.0, min_iter, sweeps: SmoothSolverSweeps { n_sweeps: 1, max_iter: 1000 } };
            let p = smooth_solver_sym_gauss_seidel(&a, &b, &mut x_new, &ctl);
            assert_eq!((p.iterations, p.initial_residual.to_bits(), p.final_residual.to_bits()), (sw, ini.to_bits(), fin.to_bits()), "tol {tol:e}");
            assert!(x_old.iter().zip(&x_new).all(|(u, v)| u.to_bits() == v.to_bits()), "tol {tol:e}");
        }
    }

    /// `relTol` stops at the first sweep group whose residual falls below `relTol * initial`; `nSweeps 3` counts 3 per call and smooths
    /// 3 times per residual; `maxIter` caps the count (a multiple of `nSweeps` reached or passed); negative `nSweeps` is a fixed count.
    #[test]
    fn rel_tol_n_sweeps_max_iter_and_fixed_sweeps() {
        let (a, b) = grid(9, 7);
        let base = SmoothSolverControls { tolerance: 1e-300, ..SmoothSolverControls::default() };
        let mut x = vec![0.0; a.n_cells];
        let rel = smooth_solver_sym_gauss_seidel(&a, &b, &mut x, &SmoothSolverControls { rel_tol: 0.1, ..base });
        assert!(rel.final_residual < 0.1 * rel.initial_residual && rel.iterations >= 1);
        // one sweep fewer would not have reached relTol
        let mut y = vec![0.0; a.n_cells];
        let short = smooth_solver_sym_gauss_seidel(&a, &b, &mut y, &SmoothSolverControls { sweeps: SmoothSolverSweeps { n_sweeps: 1, max_iter: rel.iterations - 1 }, ..base });
        assert!(rel.iterations == 1 || short.final_residual >= 0.1 * short.initial_residual);
        // nSweeps 3, maxIter 10: calls at 3, 6, 9, 12 -> stops at 12 (the first count >= 10)
        let mut z = vec![0.0; a.n_cells];
        let s3 = smooth_solver_sym_gauss_seidel(&a, &b, &mut z, &SmoothSolverControls { sweeps: SmoothSolverSweeps { n_sweeps: 3, max_iter: 10 }, ..base });
        assert_eq!(s3.iterations, 12);
        // the same 12 sweeps one at a time give the same solution bit for bit (the smoother restarts bPrime per sweep either way)
        let mut w = vec![0.0; a.n_cells];
        let s1 = smooth_solver_sym_gauss_seidel(&a, &b, &mut w, &SmoothSolverControls { sweeps: SmoothSolverSweeps { n_sweeps: 1, max_iter: 12 }, ..base });
        assert_eq!(s1.iterations, 12);
        assert!(z.iter().zip(&w).all(|(u, v)| u.to_bits() == v.to_bits()));
        // fixed sweeps: -4 does 4 sweeps, evaluates no residual
        let mut f = vec![0.0; a.n_cells];
        let fx = smooth_solver_sym_gauss_seidel(&a, &b, &mut f, &SmoothSolverControls { sweeps: SmoothSolverSweeps { n_sweeps: -4, max_iter: 1000 }, ..base });
        assert_eq!((fx.iterations, fx.initial_residual), (4, 0.0));
        let mut g4 = vec![0.0; a.n_cells];
        let _ = smooth_solver_sym_gauss_seidel(&a, &b, &mut g4, &SmoothSolverControls { sweeps: SmoothSolverSweeps { n_sweeps: 1, max_iter: 4 }, ..base });
        assert!(f.iter().zip(&g4).all(|(u, v)| u.to_bits() == v.to_bits()));
    }
}
