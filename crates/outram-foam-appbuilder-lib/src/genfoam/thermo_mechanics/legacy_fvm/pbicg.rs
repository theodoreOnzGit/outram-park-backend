// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
//   src/OpenFOAM/matrices/lduMatrix/solvers/PBiCG/PBiCG.C        (PBiCG::solve)
//   src/OpenFOAM/matrices/lduMatrix/preconditioners/DILUPreconditioner/DILUPreconditioner.C
//   src/OpenFOAM/matrices/lduMatrix/lduMatrix/lduMatrixSolver.C  (normFactor)
//   @ ESI OpenFOAM v2506
// Copyright (C) 2011-2016 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0 (see the crate root).
//
// This offering is not approved or endorsed by OpenCFD Limited, producer and
// distributor of the OpenFOAM(R) software via www.openfoam.com.

//! `PBiCG` with the `DILU` preconditioner, translated operation for operation from OpenFOAM-ESI
//! v2506 (gh:#1781, round 23).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! GeN-Foam's `legacyThermoMechanics` solves two kinds of asymmetric system with it: the
//! `fuelDisp`/`CRDisp` upwind transport equations (the shipped case selects `PBiCG`/`DILU`), and, as the
//! NAMED substitute for the shipped `GAMG`, the row-scaled `TStruct` equation (`(1 - isPorous) * (...)`
//! makes the matrix asymmetric; OpenFOAM refuses `PCG` for it). The whole of `PBiCG::solve` is
//! translated, including the transpose residual `rT` and `DILUPreconditioner::preconditionT`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::prelude::LduMatrix;

/// `SolverPerformance::small_`.
const SMALL: f64 = 1e-20;
/// `VSMALL` for doubles.
const VSMALL: f64 = 1e-300;

/// Result of a solve, as OpenFOAM's `solverPerformance`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PbicgPerformance {
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

fn converged(final_residual: f64, initial_residual: f64, tolerance: f64, rel_tol: f64) -> bool {
    final_residual < tolerance || (rel_tol > SMALL && final_residual < rel_tol * initial_residual)
}

/// `lduMatrix::solver::normFactor(psi, source, Apsi, tmpField)`; `sumA` as `lduMatrix::sumA`
/// (`sumA[u] += lower`, `sumA[l] += upper`).
fn norm_factor(a: &LduMatrix, psi: &[f64], source: &[f64], apsi: &[f64]) -> f64 {
    let n = psi.len();
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

/// `lduMatrix::Tmul` (transpose product): `Tpsi[c] = diag*psi`, then per face
/// `Tpsi[u] += upper*psi[l]`, `Tpsi[l] += lower*psi[u]`.
pub(crate) fn t_mul(a: &LduMatrix, x: &[f64]) -> Vec<f64> {
    let mut y: Vec<f64> = (0..a.n_cells).map(|c| a.diag[c] * x[c]).collect();
    for f in 0..a.n_internal_faces {
        y[a.neighbour[f]] += a.upper[f] * x[a.owner[f]];
        y[a.owner[f]] += a.lower[f] * x[a.neighbour[f]];
    }
    y
}

/// `DILUPreconditioner::calcReciprocalD`.
pub(crate) fn dilu_reciprocal_diag(a: &LduMatrix) -> Vec<f64> {
    let mut r_d = a.diag.clone();
    for f in 0..a.n_internal_faces {
        r_d[a.neighbour[f]] -= a.upper[f] * a.lower[f] / r_d[a.owner[f]];
    }
    for d in r_d.iter_mut() {
        *d = 1.0 / *d;
    }
    r_d
}

/// `DILUPreconditioner::precondition`.
pub(crate) fn dilu_precondition(a: &LduMatrix, r_d: &[f64], r_a: &[f64], w_a: &mut [f64]) {
    for c in 0..a.n_cells {
        w_a[c] = r_d[c] * r_a[c];
    }
    for f in 0..a.n_internal_faces {
        w_a[a.neighbour[f]] -= r_d[a.neighbour[f]] * a.lower[f] * w_a[a.owner[f]];
    }
    for f in (0..a.n_internal_faces).rev() {
        w_a[a.owner[f]] -= r_d[a.owner[f]] * a.upper[f] * w_a[a.neighbour[f]];
    }
}

/// `DILUPreconditioner::preconditionT`. The reverse sweep runs over `losort` (faces sorted by
/// neighbour cell); on an upper-triangularly ordered mesh that list is a permutation of the faces
/// ordered by `(neighbour, face)`, reproduced here with a stable sort.
pub(crate) fn dilu_precondition_t(a: &LduMatrix, losort: &[usize], r_d: &[f64], r_t: &[f64], w_t: &mut [f64]) {
    for c in 0..a.n_cells {
        w_t[c] = r_d[c] * r_t[c];
    }
    for f in 0..a.n_internal_faces {
        w_t[a.neighbour[f]] -= r_d[a.neighbour[f]] * a.upper[f] * w_t[a.owner[f]];
    }
    for k in (0..a.n_internal_faces).rev() {
        let sf = losort[k];
        w_t[a.owner[sf]] -= r_d[a.owner[sf]] * a.lower[sf] * w_t[a.neighbour[sf]];
    }
}

/// `lduAddressing::losortAddr`: the faces ordered by neighbour cell, ties in face order (upstream's
/// `calcLosort` is a counting sort over `upperAddr`, which is stable).
pub(crate) fn losort(a: &LduMatrix) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..a.n_internal_faces).collect();
    idx.sort_by_key(|&f| a.neighbour[f]);
    idx
}

/// `PBiCG::solve` with `preconditioner DILU`. `psi` is the initial guess and receives the solution.
///
/// `tolerance`, `rel_tol`, `max_iter` (default 1000) and `min_iter` (default 0) are the entries of the
/// case's `solvers` dictionary.
pub fn pbicg_dilu(
    a: &LduMatrix,
    source: &[f64],
    psi: &mut [f64],
    tolerance: f64,
    rel_tol: f64,
    max_iter: usize,
    min_iter: usize,
) -> PbicgPerformance {
    let n = psi.len();
    let wa0 = a.multiply(psi);
    let mut r_a: Vec<f64> = (0..n).map(|c| source[c] - wa0[c]).collect();
    let norm = norm_factor(a, psi, source, &wa0);
    let initial = g_sum_mag(&r_a) / norm;
    let mut perf = PbicgPerformance { iterations: 0, initial_residual: initial, final_residual: initial };
    if min_iter == 0 && converged(perf.final_residual, initial, tolerance, rel_tol) {
        return perf;
    }
    let mut p_t = vec![0.0; n];
    let w_t0 = t_mul(a, psi);
    let mut r_t: Vec<f64> = (0..n).map(|c| source[c] - w_t0[c]).collect();
    let mut w_t = w_t0;
    let mut w_a = wa0;
    let mut p_a = vec![0.0; n];
    let mut w_ar_t = 0.0_f64;
    let r_d = dilu_reciprocal_diag(a);
    let ls = losort(a);
    loop {
        let w_ar_t_old = w_ar_t;
        dilu_precondition(a, &r_d, &r_a, &mut w_a);
        dilu_precondition_t(a, &ls, &r_d, &r_t, &mut w_t);
        w_ar_t = g_sum_prod(&w_a, &r_t);
        if perf.iterations == 0 {
            p_a.copy_from_slice(&w_a);
            p_t.copy_from_slice(&w_t);
        } else {
            let beta = w_ar_t / w_ar_t_old;
            for c in 0..n {
                p_a[c] = w_a[c] + beta * p_a[c];
                p_t[c] = w_t[c] + beta * p_t[c];
            }
        }
        w_a = a.multiply(&p_a);
        w_t = t_mul(a, &p_t);
        let w_ap_t = g_sum_prod(&w_a, &p_t);
        if w_ap_t.abs() / norm < VSMALL {
            break;
        }
        let alpha = w_ar_t / w_ap_t;
        for c in 0..n {
            psi[c] += alpha * p_a[c];
            r_a[c] -= alpha * w_a[c];
            r_t[c] -= alpha * w_t[c];
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
