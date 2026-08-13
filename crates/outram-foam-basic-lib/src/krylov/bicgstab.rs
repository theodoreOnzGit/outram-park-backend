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

//! Preconditioned BiCGStab (bi-conjugate gradient stabilised).
//!
//! A short-recurrence Krylov method for **nonsymmetric** linear systems `A x = b`
//! (van der Vorst, 1992). Unlike CG it does not require `A` to be symmetric
//! positive-definite, and unlike GMRES it uses fixed (constant) work and storage
//! per iteration. It can, however, break down (the underlying bi-Lanczos process
//! can divide by a near-zero quantity); this implementation detects the two
//! classic breakdowns (`rho ≈ 0`, `omega ≈ 0`) and any `NaN`, and returns the
//! best iterate found so far with `converged = false` rather than propagating
//! garbage.
//!
//! Preconditioning is applied on both search directions (`p̂ = M^{-1} p`,
//! `ŝ = M^{-1} s`), i.e. right preconditioning of the stabilised recurrence. All
//! vectors are dimensionless `f64` of length `n_cells`.
//!
//! # Execution backend
//!
//! There is **one** implementation, [`bicgstab_prepared`], and the execution
//! backend is a parameter of it. It drives the hybrid kernels in
//! [`crate::ldu_matrix::parallel`] — [`HybridLdu::spmv_into`],
//! [`HybridLdu::residual_into`], [`dot`], [`axpy`], [`norm_l2`] — and the
//! backend-aware [`Preconditioner::apply_on`]. [`bicgstab`] is the convenience
//! adapter for a caller holding a bare [`LduMatrix`]: it builds the cell-gather
//! index and runs the same body on [`ComputeBackend::Serial`]. The two are
//! **not** a `foo()`/`foo_parallel()` pair — they differ in who owns the index,
//! not in where they run.
//!
//! # Determinism
//!
//! Every kernel this solver uses is bitwise identical between
//! [`ComputeBackend::Serial`] and [`ComputeBackend::CpuMulti`] at any thread
//! count, so **the whole solve is**: same iterates, same residual history, same
//! iteration count, bit for bit. That is a stronger parity statement than the
//! tolerance-based gate bead `op-yvj.4.4` asked for, and it is measured rather
//! than argued — see `crate::krylov::hybrid_tests`.
//!
//! What it is *not* bitwise equal to is the solver as it stood before this
//! module took the hybrid path, because [`dot`] sums in blocks of
//! [`REDUCTION_BLOCK`](crate::ldu_matrix::parallel::REDUCTION_BLOCK) where
//! [`crate::krylov::vecops::dot`] sums flat, and floating-point addition is not
//! associative. For `n <= 1024` the two are *identical* (one block, and
//! combining a single partial is exact), so every system smaller than that —
//! which includes every unit test in this module — is unchanged bit for bit. Above
//! that they differ in the last bits; the measured effect on residual histories
//! and iteration counts is recorded in `crate::krylov::hybrid_tests`.

use std::sync::Arc;

use super::{KrylovResult, KrylovSettings, Preconditioner};
use crate::compute::ComputeBackend;
use crate::ldu_matrix::parallel::{axpy, dot, norm_l2, HybridLdu};
use crate::ldu_matrix::LduMatrix;

/// Below this magnitude a BiCGStab scalar (`rho`, `omega`, or a denominator) is
/// treated as a breakdown rather than being divided by.
const BREAKDOWN_TOL: f64 = 1.0e-30;

/// Solve `A x = b` with preconditioned BiCGStab, serially, from a bare
/// [`LduMatrix`].
///
/// The convenience adapter over [`bicgstab_prepared`]: it builds the cell-gather
/// index ([`HybridLdu::new`]) and runs on [`ComputeBackend::Serial`]. Use it when
/// you hold a one-off matrix and do not care about the backend.
///
/// # Prefer [`bicgstab_prepared`] in a solver loop
///
/// This adapter clones `a` into an [`Arc`] and builds an
/// `O(n_cells + n_internal_faces)` index on **every call**. That is negligible
/// against a solve that performs tens of matrix-vector products, but a
/// finite-volume solver reassembling the same mesh every outer iteration should
/// hold a [`HybridLdu`], refresh it with [`HybridLdu::with_matrix`], and call
/// [`bicgstab_prepared`] — which also lets it ask for
/// [`ComputeBackend::CpuMulti`].
///
/// # Arguments
/// - `a` — the sparse system matrix (LDU). May be asymmetric.
/// - `b` — right-hand side, length `n_cells`.
/// - `x0` — optional initial guess; `None` means the zero vector.
/// - `precond` — preconditioner `M^{-1}` (identity / Jacobi / ILU(0)).
/// - `settings` — tolerance (relative, on `||r||₂ / ||b||₂`) and `max_iter`;
///   `restart` is ignored by BiCGStab.
///
/// # Returns
/// `(x, result)` where `x` is the best iterate and `result` reports the iteration
/// count, the **true** relative residual `||b − A x||₂ / ||b||₂`, and whether the
/// tolerance was met. If `b` is exactly zero, returns `x = 0`, `converged = true`,
/// `0` iterations. On breakdown or `NaN`, returns the best-so-far iterate with
/// `converged = false`.
pub fn bicgstab(
    a: &LduMatrix,
    b: &[f64],
    x0: Option<&[f64]>,
    precond: &Preconditioner,
    settings: &KrylovSettings,
) -> (Vec<f64>, KrylovResult) {
    let ldu = HybridLdu::new(Arc::new(a.clone()));
    bicgstab_prepared(&ldu, b, x0, precond, settings, ComputeBackend::Serial)
}

/// Solve `A x = b` with preconditioned BiCGStab on a chosen [`ComputeBackend`].
///
/// **This is the implementation**; [`bicgstab`] is a thin adapter onto it. The
/// matrix arrives as a [`HybridLdu`], i.e. with its cell-gather index already
/// built, so the index cost is paid once per mesh rather than once per solve.
///
/// # What runs where
///
/// Per iteration the solver performs two sparse products, four inner products,
/// several `axpy`s and two preconditioner applications. Each is dispatched
/// independently through this module's measured size floors, which differ by 64x:
///
/// | Kernel | Floor for `CpuMulti` |
/// |---|---|
/// | [`HybridLdu::spmv_into`], [`HybridLdu::residual_into`] | [`SPMV_MIN_CELLS`](crate::ldu_matrix::parallel::SPMV_MIN_CELLS) = 4 096 cells |
/// | [`dot`], [`axpy`], [`norm_l2`], Jacobi apply | [`VECOP_MIN_ELEMENTS`](crate::ldu_matrix::parallel::VECOP_MIN_ELEMENTS) = 262 144 elements |
///
/// So on a mesh between those two sizes — which is most meshes a workstation
/// runs — **the products thread and the vector operations deliberately do not**.
/// That is not an oversight: the vector operations do one or two flops per element
/// loaded and lose to thread-dispatch overhead until the vector is very large,
/// whereas the gather product does roughly seven and pays for itself two orders
/// of magnitude earlier.
///
/// # Measured end-to-end speed-up
///
/// Whole-solve wall clock, `Serial` against `CpuMulti`, on an asymmetric
/// 7-point-stencil system, `available_parallelism()` = 4, release,
/// `--features parallel`, 2026-08-13, best of 5 complete solves per figure.
/// Ranges span **four independent runs** at load averages between 0.59 and 2.19
/// (`end_to_end_solve_speedup_benchmark`; this host never reaches idle, so the
/// parallel columns are pessimistic):
///
/// | Cells | Jacobi-preconditioned | ILU(0)-preconditioned |
/// |---|---|---|
/// | 4 096 | **0.62-0.81x** (a loss) | **0.67-0.81x** (a loss) |
/// | 32 768 | 1.57-1.82x | 1.25-1.32x |
/// | 262 144 | 2.19-2.51x | 1.41-1.49x |
/// | 512 000 | **2.40-2.66x** | **1.50-1.51x** |
///
/// Two things a caller should take from this:
///
/// - **The product's crossover is not the solve's crossover.** At exactly
///   [`SPMV_MIN_CELLS`](crate::ldu_matrix::parallel::SPMV_MIN_CELLS) = 4 096, where
///   the isolated product breaks even, the whole solve *loses* — it interleaves
///   the product with vector operations and a preconditioner apply that all still
///   run serially at that size, so the product's marginal win is diluted while
///   the dispatch cost is paid every iteration. On this machine a solve does not
///   reliably win until roughly 13 000 cells.
/// - **ILU(0) caps the achievable speed-up**, because its triangular solves are
///   sequential and measure **29-46% of the serial solve** — an Amdahl cap of
///   1.7-2.1x on 4 cores. ILU(0) reaches ~1.5x, so it is held below even that cap
///   by ordinary parallel-efficiency loss. Jacobi, whose apply is embarrassingly
///   parallel, reaches 2.4-2.7x. If you need the parallel win more than you need
///   ILU(0)'s smaller iteration count, that trade is now quantified.
///
/// Full methodology, repeat runs and limitations are on the benchmarks in
/// `crate::krylov::hybrid_tests`.
///
/// # Determinism
///
/// Bitwise identical on [`ComputeBackend::Serial`] and
/// [`ComputeBackend::CpuMulti`], at any thread count: identical iterates,
/// identical residual history, identical iteration count. See the module
/// documentation for the one thing it is *not* bitwise equal to.
///
/// # Arguments
/// - `ldu` — the prepared sparse system. May be asymmetric.
/// - `b` — right-hand side, length `n_cells`.
/// - `x0` — optional initial guess; `None` means the zero vector.
/// - `precond` — preconditioner `M^{-1}`, built from the **same** matrix.
/// - `settings` — tolerance and `max_iter`; `restart` is ignored by BiCGStab.
/// - `backend` — requested execution backend. A backend whose feature is off, or
///   whose hardware is absent, degrades rather than failing; there is no GPU
///   kernel yet, so [`ComputeBackend::Gpu`] runs on the best CPU path.
///
/// # Returns
///
/// As [`bicgstab`].
///
/// # Example
///
/// ```rust
/// use std::sync::Arc;
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::ldu_matrix::LduMatrix;
/// use outram_foam_basic_lib::ldu_matrix::parallel::HybridLdu;
/// use outram_foam_basic_lib::krylov::{bicgstab_prepared, KrylovSettings, Preconditioner};
///
/// let mut a = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
/// a.diag = vec![4.0, 4.0, 4.0];
/// a.lower = vec![-1.0, -1.0];
/// a.upper = vec![-1.0, -1.0];
/// let precond = Preconditioner::jacobi(&a);
/// let ldu = HybridLdu::new(Arc::new(a));
/// let b = vec![1.0, 2.0, 3.0];
/// let settings = KrylovSettings::default();
///
/// let (x_ser, r_ser) =
///     bicgstab_prepared(&ldu, &b, None, &precond, &settings, ComputeBackend::Serial);
/// let (x_par, r_par) =
///     bicgstab_prepared(&ldu, &b, None, &precond, &settings, ComputeBackend::CpuMulti);
///
/// assert!(r_ser.converged);
/// // Bit-for-bit identical, including the iteration count — not merely close.
/// assert_eq!(x_ser, x_par);
/// assert_eq!(r_ser.n_iterations, r_par.n_iterations);
/// ```
pub fn bicgstab_prepared(
    ldu: &HybridLdu,
    b: &[f64],
    x0: Option<&[f64]>,
    precond: &Preconditioner,
    settings: &KrylovSettings,
    backend: ComputeBackend,
) -> (Vec<f64>, KrylovResult) {
    bicgstab_impl(ldu, b, x0, precond, settings, backend, &mut Vec::new())
}

/// [`bicgstab_prepared`] with the relative-residual history captured.
///
/// `history` is cleared and then receives one entry per completed iteration: the
/// relative residual `||r||₂ / ||b||₂` the algorithm saw at the end of that
/// iteration (for an iteration that exits early on the half-step, the half-step
/// residual). This is what a V&V comparison of two backends must check — a
/// changed preconditioner or a re-associated reduction shows up here long before
/// it shows up in the final answer — so it is kept crate-internal and driven from
/// the tests rather than added to the public surface.
pub(crate) fn bicgstab_impl(
    ldu: &HybridLdu,
    b: &[f64],
    x0: Option<&[f64]>,
    precond: &Preconditioner,
    settings: &KrylovSettings,
    backend: ComputeBackend,
    history: &mut Vec<f64>,
) -> (Vec<f64>, KrylovResult) {
    let n = ldu.matrix().n_cells;
    history.clear();
    let bnorm = norm_l2(b, backend);

    // Trivial system: b == 0 -> x == 0 is the exact solution.
    if bnorm == 0.0 {
        return (
            vec![0.0; n],
            KrylovResult {
                n_iterations: 0,
                final_residual: 0.0,
                converged: true,
            },
        );
    }

    let mut x = match x0 {
        Some(g) => g.to_vec(),
        None => vec![0.0; n],
    };

    // r = b - A x
    let mut r = ldu.residual(&x, b, backend);
    let rhat0 = r.clone(); // fixed shadow residual r̂0

    // Track the best iterate by relative residual.
    let mut best_x = x.clone();
    let mut best_rel = norm_l2(&r, backend) / bnorm;

    if best_rel <= settings.tolerance {
        return (
            best_x,
            KrylovResult {
                n_iterations: 0,
                final_residual: best_rel,
                converged: true,
            },
        );
    }

    let mut rho_old: f64 = 1.0;
    let mut alpha: f64 = 1.0;
    let mut omega: f64 = 1.0;
    // Scratch buffers, allocated once and reused: the whole point of taking a
    // prepared matrix is that the inner loop touches no allocator.
    let mut v = vec![0.0; n];
    let mut p = vec![0.0; n];
    let mut phat = vec![0.0; n];
    let mut shat = vec![0.0; n];
    let mut s = vec![0.0; n];
    let mut t = vec![0.0; n];
    let mut scratch = vec![0.0; n];

    let mut iters = 0usize;
    let mut converged = false;

    while iters < settings.max_iter {
        iters += 1;

        let rho = dot(&rhat0, &r, backend);
        if !rho.is_finite() || rho.abs() < BREAKDOWN_TOL {
            break; // rho breakdown
        }

        // beta = (rho/rho_old) * (alpha/omega)
        if omega.abs() < BREAKDOWN_TOL {
            break;
        }
        let beta = (rho / rho_old) * (alpha / omega);

        // p = r + beta*(p - omega*v)
        for i in 0..n {
            p[i] = r[i] + beta * (p[i] - omega * v[i]);
        }

        // phat = M^{-1} p ; v = A phat
        precond.apply_on(&p, &mut phat, backend);
        ldu.spmv_into(&phat, &mut v, backend);

        let rhat_v = dot(&rhat0, &v, backend);
        if !rhat_v.is_finite() || rhat_v.abs() < BREAKDOWN_TOL {
            break;
        }
        alpha = rho / rhat_v;

        // s = r - alpha*v
        s.copy_from_slice(&r);
        axpy(-alpha, &v, &mut s, backend);

        // Early convergence on the half-step: x += alpha*phat.
        let s_rel = norm_l2(&s, backend) / bnorm;
        if s_rel <= settings.tolerance {
            history.push(s_rel);
            axpy(alpha, &phat, &mut x, backend);
            ldu.residual_into(&x, b, &mut scratch, backend);
            let rel = norm_l2(&scratch, backend) / bnorm;
            if rel < best_rel {
                best_rel = rel;
                best_x.copy_from_slice(&x);
            }
            converged = best_rel <= settings.tolerance;
            break;
        }

        // shat = M^{-1} s ; t = A shat
        precond.apply_on(&s, &mut shat, backend);
        ldu.spmv_into(&shat, &mut t, backend);

        let tt = dot(&t, &t, backend);
        if !tt.is_finite() || tt.abs() < BREAKDOWN_TOL {
            // omega undefined: still take the alpha step before bailing.
            axpy(alpha, &phat, &mut x, backend);
            ldu.residual_into(&x, b, &mut scratch, backend);
            let rel = norm_l2(&scratch, backend) / bnorm;
            if rel < best_rel {
                best_rel = rel;
                best_x.copy_from_slice(&x);
            }
            history.push(rel);
            break;
        }
        omega = dot(&t, &s, backend) / tt;

        // x += alpha*phat + omega*shat
        axpy(alpha, &phat, &mut x, backend);
        axpy(omega, &shat, &mut x, backend);

        // r = s - omega*t
        r.copy_from_slice(&s);
        axpy(-omega, &t, &mut r, backend);

        let rel = norm_l2(&r, backend) / bnorm;
        history.push(rel);
        if !rel.is_finite() {
            break; // NaN/inf guard: keep best-so-far
        }
        if rel < best_rel {
            best_rel = rel;
            best_x.copy_from_slice(&x);
        }
        if best_rel <= settings.tolerance {
            converged = true;
            break;
        }

        if omega.abs() < BREAKDOWN_TOL {
            break; // omega breakdown
        }
        rho_old = rho;
    }

    // Report the TRUE residual of the returned iterate.
    ldu.residual_into(&best_x, b, &mut scratch, backend);
    let true_rel = norm_l2(&scratch, backend) / bnorm;
    let true_rel = if true_rel.is_finite() {
        true_rel
    } else {
        best_rel
    };
    (
        best_x,
        KrylovResult {
            n_iterations: iters,
            final_residual: true_rel,
            converged: converged && true_rel <= settings.tolerance,
        },
    )
}
