// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2011-2017 OpenFOAM Foundation
// Copyright (C) 2019-2023 OpenCFD Ltd.
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

//! `PBiCGStab` with the `DILU` preconditioner, in OpenFOAM-v2506 arithmetic.
//!
//! C++ sources (ESI v2506): `src/OpenFOAM/matrices/lduMatrix/solvers/PBiCGStab/PBiCGStab.C`
//! (`scalarSolve`) and `preconditioners/DILUPreconditioner/DILUPreconditioner.C`
//! (`calcReciprocalD`, `precondition`).
//!
//! Why a separate function: the crate's [`crate::krylov::bicgstab`] uses an `ILU0`
//! preconditioner, which also modifies the off-diagonal factors; upstream's `DILU`
//! modifies only the diagonal (`rD`). The two give different iterates from the first
//! iteration, so a code-to-code comparison against OpenFOAM's `PBiCGStab`/`DILU`
//! (gh:#1362 row 3) needs this form. The operation order follows upstream so the
//! iterates can be bit-identical.
//!
//! Interfaces (coupled patches) do not exist in [`LduMatrix`]. Parallel reductions
//! are the serial sums.

use crate::ldu_matrix::ldu_matrix::LduMatrix;

/// Controls of [`pbicgstab_dilu`] (upstream `tolerance`, `relTol`, `minIter`, `maxIter`).
#[derive(Debug, Clone, Copy)]
pub struct PbicgstabDiluSettings {
    /// Absolute tolerance on the normalised residual (upstream default 1e-6).
    pub tolerance: f64,
    /// Relative tolerance on the initial residual (0 disables).
    pub rel_tol: f64,
    /// Minimum number of iterations.
    pub min_iter: usize,
    /// Maximum number of iterations (upstream default 1000).
    pub max_iter: usize,
}

/// Outcome of [`pbicgstab_dilu`].
#[derive(Debug, Clone, Copy)]
pub struct PbicgstabDiluPerformance {
    /// Iterations performed.
    pub n_iterations: usize,
    /// Upstream-normalised initial residual `sum|b - A x| / normFactor`.
    pub initial_residual: f64,
    /// Upstream-normalised final residual (from the recurrence residual, as upstream).
    pub final_residual: f64,
    /// Whether the tolerance or relative tolerance was met.
    pub converged: bool,
}

const SMALL: f64 = 1e-20;
const VSMALL: f64 = 1e-300;

/// Upstream `DILUPreconditioner::calcReciprocalD`.
fn dilu_reciprocal_diag(ldu: &LduMatrix) -> Vec<f64> {
    let mut r_d = ldu.diag.clone();
    for f in 0..ldu.n_internal_faces {
        r_d[ldu.neighbour[f]] -= ldu.upper[f] * ldu.lower[f] / r_d[ldu.owner[f]];
    }
    for d in r_d.iter_mut() {
        *d = 1.0 / *d;
    }
    r_d
}

/// Upstream `DILUPreconditioner::precondition`: `w = M^-1 r`.
fn dilu_precondition(ldu: &LduMatrix, r_d: &[f64], r: &[f64], w: &mut [f64]) {
    for c in 0..ldu.n_cells {
        w[c] = r_d[c] * r[c];
    }
    for f in 0..ldu.n_internal_faces {
        let (l, u) = (ldu.owner[f], ldu.neighbour[f]);
        w[u] -= r_d[u] * ldu.lower[f] * w[l];
    }
    for f in (0..ldu.n_internal_faces).rev() {
        let (l, u) = (ldu.owner[f], ldu.neighbour[f]);
        w[l] -= r_d[l] * ldu.upper[f] * w[u];
    }
}

fn sum_mag(a: &[f64]) -> f64 {
    let mut s = 0.0;
    for v in a {
        s += v.abs();
    }
    s
}

fn sum_prod(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for i in 0..a.len() {
        s += a[i] * b[i];
    }
    s
}

/// Upstream `lduMatrix::solver::normFactor` (no interfaces).
fn norm_factor(ldu: &LduMatrix, psi: &[f64], source: &[f64], ax: &[f64]) -> f64 {
    let n = ldu.n_cells;
    let ones = vec![1.0_f64; n];
    let sum_a = ldu.multiply(&ones);
    let mut s = 0.0;
    for v in psi {
        s += v;
    }
    let x_ref = s / n as f64;
    let mut norm = 0.0;
    for c in 0..n {
        let pa = sum_a[c] * x_ref;
        norm += (ax[c] - pa).abs() + (source[c] - pa).abs();
    }
    norm + SMALL
}

fn check_convergence(res: f64, init: f64, s: &PbicgstabDiluSettings) -> bool {
    res < s.tolerance || (s.rel_tol > SMALL && res < s.rel_tol * init)
}

/// Solve `A x = b` with upstream's `PBiCGStab` and `DILU` from the guess `x0`.
///
/// # Arguments
/// * `ldu` - matrix (its `lower` is used as stored, so a symmetric matrix must have
///   `lower == upper` filled in).
/// * `b` - right-hand side.
/// * `x0` - initial guess.
/// * `settings` - tolerances and iteration limits.
///
/// Returns the iterate and its performance.
pub fn pbicgstab_dilu(
    ldu: &LduMatrix,
    b: &[f64],
    x0: &[f64],
    settings: &PbicgstabDiluSettings,
) -> (Vec<f64>, PbicgstabDiluPerformance) {
    let n = ldu.n_cells;
    let mut psi = x0.to_vec();
    let mut y_a = ldu.multiply(&psi);
    let mut r_a: Vec<f64> = (0..n).map(|c| b[c] - y_a[c]).collect();
    let nf = norm_factor(ldu, &psi, b, &y_a);
    let initial = sum_mag(&r_a) / nf;
    let mut perf = PbicgstabDiluPerformance {
        n_iterations: 0,
        initial_residual: initial,
        final_residual: initial,
        converged: false,
    };
    if settings.min_iter > 0 || !check_convergence(perf.final_residual, initial, settings) {
        let r_d = dilu_reciprocal_diag(ldu);
        let mut p_a = vec![0.0; n];
        let mut ay_a = vec![0.0; n];
        let mut s_a = vec![0.0; n];
        let mut z_a = vec![0.0; n];
        let r_a0 = r_a.clone();
        let (mut r_a0_r_a, mut alpha, mut omega) = (0.0_f64, 0.0_f64, 0.0_f64);
        loop {
            let r_a0_r_a_old = r_a0_r_a;
            r_a0_r_a = sum_prod(&r_a0, &r_a);
            if r_a0_r_a.abs() < VSMALL {
                break;
            }
            if perf.n_iterations == 0 {
                p_a.copy_from_slice(&r_a);
            } else {
                if omega.abs() < VSMALL {
                    break;
                }
                let beta = (r_a0_r_a / r_a0_r_a_old) * (alpha / omega);
                for c in 0..n {
                    p_a[c] = r_a[c] + beta * (p_a[c] - omega * ay_a[c]);
                }
            }
            dilu_precondition(ldu, &r_d, &p_a, &mut y_a);
            ay_a = ldu.multiply(&y_a);
            let r_a0_ay_a = sum_prod(&r_a0, &ay_a);
            alpha = r_a0_r_a / r_a0_ay_a;
            for c in 0..n {
                s_a[c] = r_a[c] - alpha * ay_a[c];
            }
            perf.final_residual = sum_mag(&s_a) / nf;
            if perf.n_iterations >= settings.min_iter
                && check_convergence(perf.final_residual, initial, settings)
            {
                for c in 0..n {
                    psi[c] += alpha * y_a[c];
                }
                perf.n_iterations += 1;
                perf.converged = true;
                return (psi, perf);
            }
            dilu_precondition(ldu, &r_d, &s_a, &mut z_a);
            let t_a = ldu.multiply(&z_a);
            let t_a_t_a = sum_prod(&t_a, &t_a);
            omega = sum_prod(&t_a, &s_a) / t_a_t_a;
            for c in 0..n {
                psi[c] += alpha * y_a[c] + omega * z_a[c];
                r_a[c] = s_a[c] - omega * t_a[c];
            }
            perf.final_residual = sum_mag(&r_a) / nf;
            perf.n_iterations += 1;
            let again = (perf.n_iterations < settings.max_iter
                && !check_convergence(perf.final_residual, initial, settings))
                || perf.n_iterations < settings.min_iter;
            if !again {
                break;
            }
        }
    }
    perf.converged = check_convergence(perf.final_residual, initial, settings);
    (psi, perf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn laplace_1d(n: usize) -> (LduMatrix, Vec<f64>) {
        let mut m = LduMatrix::new(n, (0..n - 1).collect(), (1..n).collect());
        m.diag = vec![2.0; n];
        m.upper = vec![-1.0; n - 1];
        m.lower = vec![-1.0; n - 1];
        (m, vec![1.0; n])
    }

    #[test]
    fn solves_a_1d_laplacian_to_tolerance() {
        let (m, b) = laplace_1d(40);
        let s = PbicgstabDiluSettings { tolerance: 1e-10, rel_tol: 0.0, min_iter: 0, max_iter: 200 };
        let (x, perf) = pbicgstab_dilu(&m, &b, &vec![0.0; 40], &s);
        assert!(perf.converged, "{perf:?}");
        let r = m.residual(&x, &b);
        let rn: f64 = r.iter().map(|v| v * v).sum::<f64>().sqrt();
        assert!(rn < 1e-7, "residual {rn}");
    }

    #[test]
    fn fixed_iteration_count_is_honoured_with_zero_tolerance() {
        let (m, b) = laplace_1d(40);
        let s = PbicgstabDiluSettings { tolerance: 0.0, rel_tol: 0.0, min_iter: 3, max_iter: 3 };
        let (_, perf) = pbicgstab_dilu(&m, &b, &vec![0.0; 40], &s);
        assert_eq!(perf.n_iterations, 3);
    }
}
