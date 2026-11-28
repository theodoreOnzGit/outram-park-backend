// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! `PCG` with the `DIC` preconditioner, translated operation for operation
//! from OpenFOAM-v2506 (gh:#1361 round 17, gh:#1833).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! # Upstream
//!
//! - `src/OpenFOAM/matrices/lduMatrix/solvers/PCG/PCG.C`, `PCG::scalarSolve`
//!   (the whole function);
//! - `.../preconditioners/DICPreconditioner/DICPreconditioner.C`
//!   (`calcReciprocalD`, `precondition`);
//! - `SolverPerformance::checkConvergence` / `checkSingularity`
//!   (`small_ = 1e-20`, `vsmall_ = VSMALL`) and `lduMatrix::solver::normFactor`.
//!
//! # Why this exists
//!
//! The two-phase `p_rgh` equation with `p ~ 1.5e7 Pa` and a pressure variation of
//! `3e4 Pa` is ill-conditioned: a linear solve whose residual is small is not
//! thereby accurate in `p_rgh`. The port solved it with an ILU(0)-BiCGStab whose
//! stopping rule is `||r||_2 / ||b||_2 < 1e-13` (`solve_krylov`), where `b` is
//! dominated by the `fixedValue` outlet term; on `PhaseII_Ex3` that left
//! `p_rgh` wrong by 2.7e-3 Pa after the first corrector (the exact solution of the
//! port's own matrix, solved in 40-digit arithmetic, differs from the port's
//! iterate by that much and from the real binary's `DICPCG` iterate by 1.3e-5 Pa),
//! growing to `p` 5e-6 and `U` 2e-5 relative at the end of the step. Running the
//! upstream algorithm with the upstream stopping rule removes it.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::prelude::LduMatrix;
use super::ldu_interfaces::{amul, LduInterfaces};

/// `SolverPerformance::small_`.
const SMALL: f64 = 1e-20;
/// `VSMALL` for doubles.
const VSMALL: f64 = 1e-300;

/// Result of a solve, as OpenFOAM's `solverPerformance`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PcgPerformance {
    /// Iterations performed.
    pub iterations: usize,
    /// Normalised residual before the first iteration.
    pub initial_residual: f64,
    /// Normalised residual at exit.
    pub final_residual: f64,
}

pub(crate) fn g_sum_mag(v: &[f64]) -> f64 {
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
pub(crate) fn converged(final_residual: f64, initial_residual: f64, tolerance: f64, rel_tol: f64) -> bool {
    final_residual < tolerance || (rel_tol > SMALL && final_residual < rel_tol * initial_residual)
}

/// `lduMatrix::solver::normFactor(psi, source, Apsi, tmpField)`.
pub(crate) fn norm_factor(a: &LduMatrix, psi: &[f64], source: &[f64], apsi: &[f64]) -> f64 {
    norm_factor_if(a, &LduInterfaces::none(), psi, source, apsi)
}

/// [`norm_factor`] with coupled interfaces: `sumA` subtracts the `interfaceBouCoeffs` after the face loop (`lduMatrix::sumA`,
/// gh:#1905 round 80).
pub(crate) fn norm_factor_if(a: &LduMatrix, ifs: &LduInterfaces, psi: &[f64], source: &[f64], apsi: &[f64]) -> f64 {
    let n = psi.len();
    let mut sum_a = a.diag.clone();
    for f in 0..a.n_internal_faces {
        sum_a[a.neighbour[f]] += a.lower[f];
        sum_a[a.owner[f]] += a.upper[f];
    }
    ifs.sum_a_update(&mut sum_a);
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

/// `DICPreconditioner::calcReciprocalD`.
pub(crate) fn dic_reciprocal_diag(a: &LduMatrix) -> Vec<f64> {
    let mut r_d = a.diag.clone();
    for f in 0..a.n_internal_faces {
        r_d[a.neighbour[f]] -= a.upper[f] * a.upper[f] / r_d[a.owner[f]];
    }
    for d in r_d.iter_mut() {
        *d = 1.0 / *d;
    }
    r_d
}

/// `DICPreconditioner::precondition`.
fn dic_precondition(a: &LduMatrix, r_d: &[f64], r_a: &[f64], w_a: &mut [f64]) {
    let n = w_a.len();
    for c in 0..n {
        w_a[c] = r_d[c] * r_a[c];
    }
    for f in 0..a.n_internal_faces {
        let u = a.neighbour[f];
        w_a[u] -= r_d[u] * a.upper[f] * w_a[a.owner[f]];
    }
    for f in (0..a.n_internal_faces).rev() {
        let l = a.owner[f];
        w_a[l] -= r_d[l] * a.upper[f] * w_a[a.neighbour[f]];
    }
}

/// `PCG::scalarSolve` with `preconditioner DIC`. `psi` is the initial guess and
/// receives the solution. The matrix must be symmetric with its faces in upper
/// triangular order (what a `fvm::laplacian` assembles on an OpenFOAM mesh).
///
/// `tolerance`, `rel_tol`, `max_iter` (default 1000) and `min_iter` (default 0)
/// are the entries of the case's `solvers` dictionary.
pub(crate) fn pcg_dic(
    a: &LduMatrix,
    source: &[f64],
    psi: &mut [f64],
    tolerance: f64,
    rel_tol: f64,
    max_iter: usize,
    min_iter: usize,
) -> PcgPerformance {
    pcg_dic_if(a, &LduInterfaces::none(), source, psi, tolerance, rel_tol, max_iter, min_iter)
}

/// [`pcg_dic`] with coupled interfaces (gh:#1905 round 80): `Amul` and `normFactor` carry them, the `DIC` preconditioner does not
/// (`PCG.C`, `DICPreconditioner.C`). With [`LduInterfaces::none`] the arithmetic is that of [`pcg_dic`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn pcg_dic_if(
    a: &LduMatrix,
    ifs: &LduInterfaces,
    source: &[f64],
    psi: &mut [f64],
    tolerance: f64,
    rel_tol: f64,
    max_iter: usize,
    min_iter: usize,
) -> PcgPerformance {
    let n = psi.len();
    let wa0 = amul(a, ifs, psi);
    let mut r_a: Vec<f64> = (0..n).map(|c| source[c] - wa0[c]).collect();
    let norm = norm_factor_if(a, ifs, psi, source, &wa0);
    let initial = g_sum_mag(&r_a) / norm;
    let mut perf = PcgPerformance { iterations: 0, initial_residual: initial, final_residual: initial };
    if min_iter == 0 && converged(perf.final_residual, initial, tolerance, rel_tol) {
        return perf;
    }
    let r_d = dic_reciprocal_diag(a);
    let mut w_a = vec![0.0; n];
    let mut p_a = vec![0.0; n];
    let mut w_ar_a = 1e15_f64;
    loop {
        let w_ar_a_old = w_ar_a;
        dic_precondition(a, &r_d, &r_a, &mut w_a);
        w_ar_a = g_sum_prod(&w_a, &r_a);
        if perf.iterations == 0 {
            p_a.copy_from_slice(&w_a);
        } else {
            let beta = w_ar_a / w_ar_a_old;
            for c in 0..n {
                p_a[c] = w_a[c] + beta * p_a[c];
            }
        }
        w_a = amul(a, ifs, &p_a);
        let w_ap_a = g_sum_prod(&w_a, &p_a);
        if w_ap_a.abs() / norm < VSMALL {
            break;
        }
        let alpha = w_ar_a / w_ap_a;
        for c in 0..n {
            psi[c] += alpha * p_a[c];
            r_a[c] -= alpha * w_a[c];
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

    /// A 2-D five-point Laplacian on an `nx` x `ny` grid with a Dirichlet row folded into the diagonal (symmetric SPD, faces in upper
    /// triangular order) and the exact solution `x_c = c`. DIC is NOT exact here (it is on a 1-D chain), so the iteration count is a real test.
    fn grid(nx: usize, ny: usize) -> (LduMatrix, Vec<f64>, Vec<f64>) {
        let n = nx * ny;
        let mut owner = Vec::new();
        let mut neighbour = Vec::new();
        for c in 0..n {
            if (c + 1) % nx != 0 {
                owner.push(c);
                neighbour.push(c + 1);
            }
            if c + nx < n {
                owner.push(c);
                neighbour.push(c + nx);
            }
        }
        let nf = owner.len();
        let mut a = LduMatrix::new(n, owner, neighbour);
        a.diag = vec![4.0; n];
        a.upper = vec![-1.0; nf];
        a.lower = vec![-1.0; nf];
        let x: Vec<f64> = (0..n).map(|i| i as f64).collect();
        let b = a.multiply(&x);
        (a, b, x)
    }

    /// A tight tolerance reaches the exact solution, in more than one iteration (DIC is inexact on a 2-D grid) and at most `n`.
    #[test]
    fn solves_a_symmetric_grid() {
        let (a, b, x_exact) = grid(8, 8);
        let mut x = vec![0.0; 64];
        let perf = pcg_dic(&a, &b, &mut x, 1e-14, 0.0, 1000, 0);
        assert!(perf.final_residual < 1e-14, "{perf:?}");
        assert!(perf.iterations > 1 && perf.iterations <= 64, "{perf:?}");
        for i in 0..64 {
            assert!((x[i] - x_exact[i]).abs() < 1e-9, "cell {i}");
        }
    }

    /// An already-converged guess is returned untouched after zero iterations (`minIter` 0); `minIter` forces iterations from a start
    /// that is not converged although the tolerance is loose; an exactly converged guess with `minIter` stops at the singularity check.
    #[test]
    fn converged_guess_and_min_iter() {
        let (a, b, x_exact) = grid(6, 6);
        let mut x = x_exact.clone();
        let perf = pcg_dic(&a, &b, &mut x, 1e-10, 0.0, 1000, 0);
        assert_eq!(perf.iterations, 0);
        assert_eq!(x, x_exact);
        let mut y = vec![0.0; 36];
        let free = pcg_dic(&a, &b, &mut y, 0.5, 0.0, 1000, 0);
        let mut z = vec![0.0; 36];
        let forced = pcg_dic(&a, &b, &mut z, 0.5, 0.0, 1000, free.iterations + 2);
        assert!(forced.iterations >= free.iterations + 2, "{free:?} {forced:?}");
        let mut w = x_exact.clone();
        assert_eq!(pcg_dic(&a, &b, &mut w, 1e-10, 0.0, 1000, 3).iterations, 0);
    }

    /// `relTol` stops the solve at a fraction of the initial residual, before `tolerance` is reached; `maxIter` caps the count.
    #[test]
    fn rel_tol_stops_early_and_max_iter_caps() {
        let (a, b, _) = grid(10, 10);
        let mut x = vec![0.0; 100];
        let tight = pcg_dic(&a, &b, &mut x, 1e-14, 0.0, 1000, 0);
        let mut y = vec![0.0; 100];
        let rel = pcg_dic(&a, &b, &mut y, 1e-14, 0.1, 1000, 0);
        assert!(rel.iterations < tight.iterations);
        assert!(rel.final_residual < 0.1 * rel.initial_residual);
        let mut z = vec![0.0; 100];
        assert_eq!(pcg_dic(&a, &b, &mut z, 1e-14, 0.0, 2, 0).iterations, 2);
    }
}
