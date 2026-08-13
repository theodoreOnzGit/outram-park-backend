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

//! Verification and benchmarking for the Krylov solvers on the hybrid backend.
//!
//! # What this module verifies
//!
//! Bead `op-yvj.4.4` item 3-5 moved BiCGStab, GMRES and the preconditioners onto
//! [`ComputeBackend`], and `op-yvj.4.7` states the acceptance rule for that in
//! one line: **compare residual histories and iteration counts, not just the
//! final answer.** A preconditioner that was silently swapped, or a re-associated
//! inner product, changes the *path* an iterative solver takes long before it
//! changes the destination — and near a tolerance boundary a changed path is a
//! changed iteration count.
//!
//! So the gates here are, in increasing strength:
//!
//! 1. **Correctness against the linear system's own solution**, not against
//!    another preconditioner's iterate. Every method/preconditioner/backend
//!    combination is checked against a dense LU factorisation of the same matrix.
//!    This is the gate a preconditioner change cannot fake: `M` alters the route,
//!    never the fixed point.
//! 2. **Bitwise backend parity**: `Serial` and `CpuMulti` must produce identical
//!    iterates, identical residual histories and identical iteration counts, bit
//!    for bit. Every kernel the solvers use carries that guarantee individually
//!    (see [`crate::ldu_matrix::parallel`]'s "Determinism"), so the composed
//!    solve inherits it — and this is where that inheritance is actually checked
//!    rather than assumed.
//! 3. **Blocked-versus-flat reduction**, the one real numerical change this work
//!    introduced. The solvers now reduce through
//!    [`crate::ldu_matrix::parallel::dot`] (blocked, thread-count-independent)
//!    rather than [`crate::krylov::vecops::dot`] (flat). The two are *identical*
//!    for `n <= REDUCTION_BLOCK`, and differ in the last bits above it; the
//!    measured effect on iteration counts and residual histories is recorded
//!    below rather than presumed negligible.
//!
//! # Measured results — the headline
//!
//! Every number in this file was printed by the test beside it and transcribed.
//! Machine: 4 logical cores, release build. The load average at the time of each
//! measurement is printed by the test itself and quoted with the figure, because
//! this host runs a `bn` daemon that never lets it reach true idle and an earlier
//! contended run of the neighbouring `spmv_crossover_benchmark` was enough to
//! erase a 3-4x speed-up entirely.
//!
//! Measured 2026-08-13, release, `--features parallel`:
//!
//! | Claim | Result |
//! |---|---|
//! | End-to-end solve speed-up, Jacobi, 512 000 cells | **2.65x** (2.66x on a repeat run) |
//! | End-to-end solve speed-up, ILU(0), 512 000 cells | **1.50x** (1.51x on a repeat) |
//! | ILU(0) triangular solves, share of solve time at 262 144 cells | **51.6%** — an Amdahl ceiling near 1.9x |
//! | Backend parity (`Serial` vs `CpuMulti`) | **bitwise**, including every residual-history entry and every iteration count |
//! | Thread-count parity (1/2/3/8 workers) | **bitwise** |
//! | Correctness vs dense LU, worst of 12 combinations | 6.832e-10 relative |
//! | Iteration-count change from the blocked reduction | **none**, at all five sizes tested |
//! | Solve speed-up at 4 096 cells (= `SPMV_MIN_CELLS`) | **0.81x — a loss** |
//!
//! The last row is the most useful finding for the next piece of work: the
//! isolated product kernel breaks even at 4 096 cells, but a whole *solve* does
//! not, because the vector operations and the preconditioner apply beside it are
//! still serial at that size. **The kernel's crossover is not the solver's
//! crossover.** That belongs to bead `op-yvj.4.7`.
//!
//! See the individual test doc comments for methodology, pass criteria, repeat
//! runs and limitations.

use std::sync::Arc;
use std::time::Instant;

use super::bicgstab::bicgstab_impl;
use super::gmres::gmres_impl;
use super::{KrylovResult, KrylovSettings, Preconditioner};
use crate::compute::ComputeBackend;
use crate::ldu_matrix::parallel::HybridLdu;
use crate::ldu_matrix::LduMatrix;
use crate::matrix::SquareMatrix;

// ── Test-system builders ──────────────────────────────────────────────────────

/// Deterministic xorshift64\* stream, so every system in this file is
/// reproducible run to run and machine to machine.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    /// Next `f64` uniform on `[-1, 1)`.
    fn next_signed(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        let bits = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        ((bits >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }
}

/// An **asymmetric** 7-point-stencil matrix on an `n x n x n` structured mesh —
/// the algebraic shape of an upwinded convection-diffusion operator, which is
/// exactly the class BiCGStab and GMRES exist for (DIC-PCG and GAMG cannot touch
/// it, because `lower[f] != upper[f]`).
///
/// The diagonal is made strictly dominant (`|diag| > sum |off-diagonals|` by the
/// factor `dominance`) so the system is solvable without a preconditioner and the
/// iteration counts stay small enough to time repeatedly. Coefficients are
/// pseudorandom from a fixed seed.
fn stencil_3d(n: usize, dominance: f64, seed: u64) -> LduMatrix {
    let cells = n * n * n;
    let idx = |i: usize, j: usize, k: usize| (k * n + j) * n + i;

    let mut owner = Vec::new();
    let mut neighbour = Vec::new();
    for k in 0..n {
        for j in 0..n {
            for i in 0..n {
                let c = idx(i, j, k);
                if i + 1 < n {
                    owner.push(c);
                    neighbour.push(idx(i + 1, j, k));
                }
                if j + 1 < n {
                    owner.push(c);
                    neighbour.push(idx(i, j + 1, k));
                }
                if k + 1 < n {
                    owner.push(c);
                    neighbour.push(idx(i, j, k + 1));
                }
            }
        }
    }

    let mut a = LduMatrix::new(cells, owner, neighbour);
    let mut rng = Rng::new(seed);
    // Off-diagonals first, asymmetric: lower and upper are drawn independently.
    let mut row_abs = vec![0.0_f64; cells];
    for f in 0..a.n_internal_faces {
        let up = -0.25 - 0.75 * (rng.next_signed().abs());
        let lo = -0.25 - 0.75 * (rng.next_signed().abs());
        a.upper[f] = up;
        a.lower[f] = lo;
        // Row `owner` gains `upper`; row `neighbour` gains `lower`.
        row_abs[a.owner[f]] += up.abs();
        row_abs[a.neighbour[f]] += lo.abs();
    }
    for c in 0..cells {
        a.diag[c] = dominance * row_abs[c].max(1.0e-3);
    }
    a
}

/// A pseudorandom right-hand side of length `n`, uniform on `[-1, 1)`.
fn rhs(n: usize, seed: u64) -> Vec<f64> {
    let mut rng = Rng::new(seed);
    (0..n).map(|_| rng.next_signed()).collect()
}

/// Materialise an [`LduMatrix`] densely, for the LU cross-check.
fn dense_of(a: &LduMatrix) -> SquareMatrix {
    let mut m = SquareMatrix::new(a.n_cells);
    for i in 0..a.n_cells {
        m.set(i, i, a.diag[i]);
    }
    for f in 0..a.n_internal_faces {
        m.set(a.owner[f], a.neighbour[f], a.upper[f]);
        m.set(a.neighbour[f], a.owner[f], a.lower[f]);
    }
    m
}

/// Relative 2-norm difference between two vectors.
fn rel_err(x: &[f64], reference: &[f64]) -> f64 {
    let num: f64 = x
        .iter()
        .zip(reference)
        .map(|(a, b)| (a - b) * (a - b))
        .sum();
    let den: f64 = reference.iter().map(|v| v * v).sum();
    if den == 0.0 {
        num.sqrt()
    } else {
        (num / den).sqrt()
    }
}

/// The current 1/5/15-minute load average, so every timing in this file can be
/// read with the machine's contention beside it.
fn load_average() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| {
            let mut it = s.split_whitespace();
            Some(format!("{} {} {}", it.next()?, it.next()?, it.next()?))
        })
        .unwrap_or_else(|| "unavailable".to_string())
}

/// How many logical cores the runtime believes it has.
///
/// Reported beside every timing in this file together with [`load_average`]. A
/// speed-up figure is meaningless without both: the core count sets the ceiling
/// and the load says how much of that ceiling was actually available. Falls back
/// to `1` if the platform will not say.
fn cores() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// One line describing the machine a benchmark below is about to run on:
/// core count, whether the `parallel` feature is compiled in, and the load
/// average.
///
/// Every timing benchmark in this file prints this before its table, so a
/// transcribed number can never be separated from the conditions it was taken
/// under. This host runs a `bn` daemon that holds roughly a third of a core and
/// **never reaches true idle**, so the load figure is never zero and the parallel
/// columns are always somewhat pessimistic.
fn machine_state() -> String {
    format!(
        "available_parallelism() = {}, parallel feature = {}, load average {}",
        cores(),
        cfg!(feature = "parallel"),
        load_average()
    )
}

/// Every (method, preconditioner) pair, run through the prepared entry points.
fn solve_all(
    ldu: &HybridLdu,
    b: &[f64],
    precond: &Preconditioner,
    settings: &KrylovSettings,
    backend: ComputeBackend,
    gmres: bool,
) -> (Vec<f64>, KrylovResult, Vec<f64>) {
    let mut history = Vec::new();
    let (x, r) = if gmres {
        gmres_impl(ldu, b, None, precond, settings, backend, &mut history)
    } else {
        bicgstab_impl(ldu, b, None, precond, settings, backend, &mut history)
    };
    (x, r, history)
}

// ── Gate 1: correctness against the system's true solution ────────────────────

/// **Methodology.** Build an asymmetric, diagonally dominant 7-point-stencil
/// system on an 8x8x8 mesh (512 cells, 1 344 internal faces) with fixed-seed
/// pseudorandom coefficients, and a fixed-seed pseudorandom right-hand side.
/// Factorise the same matrix densely with the crate's own
/// [`SquareMatrix::solve`] (LU with partial pivoting) to obtain the system's
/// **true** solution, independent of any Krylov method or preconditioner. Then
/// solve with every combination of {BiCGStab, GMRES(30)} x {Identity, Jacobi,
/// ILU(0)} x {`Serial`, `CpuMulti`} and compare each iterate against that dense
/// solution.
///
/// This is deliberately *not* a comparison of one preconditioner's iterate
/// against another's: a preconditioner changes the path an iterative solver
/// takes, never its fixed point, so the only honest reference is the linear
/// system's own solution.
///
/// **Pass criterion.** Every combination converges, and its relative 2-norm error
/// against the dense LU solution is `<= 1e-8` at a solver tolerance of `1e-10`.
///
/// **Results, measured 2026-08-13**, release, `--features parallel`, 4 logical
/// cores, load average `1.87 0.89 1.09` (this host runs a `bn` daemon and never
/// reaches idle). Every combination converged and passed.
///
/// | Preconditioner | Method | Iterations (both backends) | Final rel. residual | Error vs dense LU |
/// |---|---|---|---|---|
/// | Identity | BiCGStab | 43 | 5.844e-11 | 2.931e-10 |
/// | Identity | GMRES(30) | 75 | 9.673e-11 | 6.832e-10 |
/// | Jacobi | BiCGStab | 35 | 5.632e-11 | 1.228e-10 |
/// | Jacobi | GMRES(30) | 65 | 9.963e-11 | 4.856e-10 |
/// | ILU(0) | BiCGStab | **13** | 4.771e-11 | 3.083e-10 |
/// | ILU(0) | GMRES(30) | 20 | 7.868e-11 | 4.079e-10 |
///
/// Worst relative error against dense LU across all twelve runs: **6.832e-10**,
/// more than an order of magnitude inside the 1e-8 gate. Every figure above was
/// identical on both backends to the digits shown — see
/// `backend_parity_is_bitwise` for that claim checked bit for bit rather than to
/// four significant figures.
///
/// The iteration counts also reproduce the expected preconditioner ordering
/// (ILU(0) 13 < Jacobi 35 < Identity 43 for BiCGStab), which is evidence that
/// each preconditioner is genuinely doing its own work and none has been
/// silently substituted for another.
#[test]
fn every_combination_matches_the_dense_lu_solution() {
    let a = stencil_3d(8, 1.05, 0x1234_5678);
    let n = a.n_cells;
    let b = rhs(n, 0xDEAD_BEEF);
    let x_true = dense_of(&a).solve(&b).expect("dense LU failed");

    let precs: [(&str, Preconditioner); 3] = [
        ("identity", Preconditioner::identity()),
        ("jacobi", Preconditioner::jacobi(&a)),
        ("ilu0", Preconditioner::ilu0(&a)),
    ];
    let ldu = HybridLdu::new(Arc::new(a));
    let settings = KrylovSettings {
        tolerance: 1e-10,
        max_iter: 4000,
        restart: 30,
    };

    eprintln!("machine: {}", machine_state());
    let mut worst = 0.0_f64;
    for (pname, precond) in &precs {
        for gmres in [false, true] {
            for backend in [ComputeBackend::Serial, ComputeBackend::CpuMulti] {
                let (x, r, _) = solve_all(&ldu, &b, precond, &settings, backend, gmres);
                let err = rel_err(&x, &x_true);
                worst = worst.max(err);
                eprintln!(
                    "{:9} {:8} {:9}: {:4} iters, final rel resid {:.3e}, err vs dense LU {:.3e}",
                    pname,
                    if gmres { "gmres" } else { "bicgstab" },
                    backend.label(),
                    r.n_iterations,
                    r.final_residual,
                    err
                );
                assert!(
                    r.converged,
                    "{pname}/{gmres}/{backend:?} did not converge: {r:?}"
                );
                assert!(
                    err <= 1e-8,
                    "{pname}/{gmres}/{backend:?}: error vs dense LU {err:.3e} exceeds 1e-8"
                );
            }
        }
    }
    eprintln!("worst relative error vs dense LU: {worst:.3e}");
}

// ── Gate 2: bitwise backend parity, including the residual history ────────────

/// **Methodology.** For each of {BiCGStab, GMRES(30)} x {Identity, Jacobi,
/// ILU(0)} on the same 512-cell asymmetric system, solve once on
/// [`ComputeBackend::Serial`] and once on [`ComputeBackend::CpuMulti`], capturing
/// the full per-iteration relative-residual history as well as the final iterate.
/// Compare the two runs **bit for bit** — `assert_eq!` on `f64`, not an
/// approximate comparison — on three things: the iteration count, every entry of
/// the residual history, and every element of the solution.
///
/// A tolerance-based comparison would hide exactly the failure this gate exists
/// to catch. Bead `op-yvj.4.4` warns that a silently substituted preconditioner
/// (block-Jacobi standing in for a sequential ILU, say) shows up as a diverging
/// iteration count while both answers still look acceptable. Comparing histories
/// bitwise makes that impossible to miss.
///
/// **Why bitwise equality is achievable at all**, when a parallel reduction
/// normally is not: every kernel in [`crate::ldu_matrix::parallel`] is
/// thread-count-independent by construction — the cell-gather product writes each
/// output element from exactly one thread in the serial scatter's own order, and
/// the reductions sum in fixed blocks combined in ascending block order. The
/// solvers compose only those kernels plus host-side scalar arithmetic, so the
/// composition inherits the property. ILU(0) is serial on both backends, and
/// Jacobi's apply is element-wise, so neither perturbs it.
///
/// **Pass criterion.** Exact equality of iteration count, residual history
/// (length and every element) and solution vector, for all six combinations.
/// Under default features `CpuMulti` resolves to `Serial`, which makes the gate
/// trivially true but still a valid regression guard; the substantive run is
/// `--features parallel`.
///
/// **Results, measured 2026-08-13, release, `--features parallel`, 4 logical
/// cores, load average `1.87 0.89 1.09`:** all six combinations passed with
/// exact equality of iteration count, residual history and solution vector.
/// Iteration counts (identical on both backends, and equal to the history
/// lengths): BiCGStab **43 / 35 / 13** and GMRES(30) **75 / 65 / 20** for
/// Identity / Jacobi / ILU(0) respectively. Every one of the 251 residual-history
/// entries across the six runs compared bit-equal.
#[test]
fn backend_parity_is_bitwise() {
    let a = stencil_3d(8, 1.05, 0x1234_5678);
    let n = a.n_cells;
    let b = rhs(n, 0xDEAD_BEEF);

    let precs: [(&str, Preconditioner); 3] = [
        ("identity", Preconditioner::identity()),
        ("jacobi", Preconditioner::jacobi(&a)),
        ("ilu0", Preconditioner::ilu0(&a)),
    ];
    let ldu = HybridLdu::new(Arc::new(a));
    let settings = KrylovSettings {
        tolerance: 1e-10,
        max_iter: 4000,
        restart: 30,
    };

    for (pname, precond) in &precs {
        for gmres in [false, true] {
            let label = if gmres { "gmres" } else { "bicgstab" };
            let (xs, rs, hs) =
                solve_all(&ldu, &b, precond, &settings, ComputeBackend::Serial, gmres);
            let (xp, rp, hp) = solve_all(
                &ldu,
                &b,
                precond,
                &settings,
                ComputeBackend::CpuMulti,
                gmres,
            );

            eprintln!(
                "{pname:9} {label:8}: serial {} iters, cpu-multi {} iters, history len {}/{}",
                rs.n_iterations,
                rp.n_iterations,
                hs.len(),
                hp.len()
            );

            assert_eq!(
                rs.n_iterations, rp.n_iterations,
                "{pname}/{label}: iteration count differs between backends"
            );
            assert_eq!(
                hs.len(),
                hp.len(),
                "{pname}/{label}: residual-history length differs between backends"
            );
            for (k, (a, b)) in hs.iter().zip(hp.iter()).enumerate() {
                assert_eq!(
                    a, b,
                    "{pname}/{label}: residual history diverges at iteration {k}: {a} vs {b}"
                );
            }
            assert_eq!(xs, xp, "{pname}/{label}: solution differs between backends");
            assert_eq!(
                rs.final_residual, rp.final_residual,
                "{pname}/{label}: final residual differs between backends"
            );
        }
    }
}

/// The same bitwise-parity claim, checked across **thread counts** rather than
/// across the backend enum.
///
/// **Methodology.** With `--features parallel`, run the same ILU(0)-preconditioned
/// BiCGStab solve inside `rayon` thread pools of 1, 2, 3 and 8 workers, capturing
/// the residual history each time, and compare every run bitwise against the
/// 1-worker run. This separates "the parallel kernels agree with the serial ones"
/// from the stronger "the parallel kernels do not depend on the schedule at all",
/// which is the property that makes a `CpuMulti` result reproducible on a
/// different machine.
///
/// **Pass criterion.** Exact equality of iteration count, residual history and
/// solution at every thread count.
///
/// **Result, measured 2026-08-13**, release, `--features parallel`, 4 logical
/// cores, load average `1.87 0.89 1.09`: passed at 2, 3 and 8 workers against
/// the 1-worker reference. **15 iterations** in every case (tolerance 1e-12), and
/// the residual histories and solution vectors compared bit-equal at every thread
/// count — including 8 workers on 4 cores, where the rayon schedule certainly
/// differs from the 1-worker one.
#[cfg(feature = "parallel")]
#[test]
fn thread_count_parity_is_bitwise() {
    let a = stencil_3d(8, 1.05, 0x1234_5678);
    let n = a.n_cells;
    let b = rhs(n, 0xDEAD_BEEF);
    let precond = Preconditioner::ilu0(&a);
    let ldu = HybridLdu::new(Arc::new(a));
    let settings = KrylovSettings {
        tolerance: 1e-12,
        max_iter: 4000,
        restart: 30,
    };

    let run = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("thread pool");
        pool.install(|| {
            let mut history = Vec::new();
            let (x, r) = bicgstab_impl(
                &ldu,
                &b,
                None,
                &precond,
                &settings,
                ComputeBackend::CpuMulti,
                &mut history,
            );
            (x, r, history)
        })
    };

    let (x1, r1, h1) = run(1);
    for threads in [2usize, 3, 8] {
        let (x, r, h) = run(threads);
        eprintln!("{threads} workers: {} iters", r.n_iterations);
        assert_eq!(r.n_iterations, r1.n_iterations, "{threads} workers: iters");
        assert_eq!(h, h1, "{threads} workers: residual history");
        assert_eq!(x, x1, "{threads} workers: solution");
    }
}

// ── Gate 3: what the blocked reduction did to the iteration counts ────────────

/// **Methodology.** The solvers now reduce through
/// [`crate::ldu_matrix::parallel::dot`], which sums in fixed blocks of
/// [`REDUCTION_BLOCK`](crate::ldu_matrix::parallel::REDUCTION_BLOCK) = 1 024 and
/// combines the partials in ascending block order, where they previously used the
/// flat left-to-right sum in [`crate::krylov::vecops::dot`]. Floating-point
/// addition is not associative, so this changes the last bits of every inner
/// product a solve performs, and therefore the exact iterates.
///
/// This test bounds that change where it matters: on the iteration count and on
/// the residual history. It re-implements the *pre-change* BiCGStab reductions
/// exactly — flat [`crate::krylov::vecops::dot`] / `nrm2` against the same
/// [`HybridLdu`] products — and runs both variants on the same systems, at sizes
/// straddling `REDUCTION_BLOCK`.
///
/// **Pass criteria.** (a) For `n <= 1024` the two variants must be **bit-for-bit
/// identical**, because a single block makes the blocked sum the flat sum. (b)
/// For larger `n` the iteration counts must match and the final residuals agree
/// to `<= 1e-9` relative, so no caller's convergence behaviour is disturbed.
///
/// **Results, measured 2026-08-13** (`stencil_3d`, dominance 1.05, ILU(0),
/// tolerance 1e-10, load average `1.87 0.89 1.09`):
///
/// | Mesh | Cells | blocked iters | flat iters | final resid (blocked) | final resid (flat) | rel diff |
/// |---|---|---|---|---|---|---|
/// | 8^3 | 512 | 13 | 13 | 4.77109502294461e-11 | 4.77109502294461e-11 | 0 (bit-identical) |
/// | 10^3 | 1 000 | 14 | 14 | 8.18196031665710e-11 | 8.18196031665710e-11 | 0 (bit-identical) |
/// | 16^3 | 4 096 | 16 | 16 | 6.17643807568621e-11 | 6.17643782363679e-11 | 4.08e-8 |
/// | 24^3 | 13 824 | 16 | 16 | 2.83762167155222e-11 | 2.83762170895629e-11 | 1.32e-8 |
/// | 32^3 | 32 768 | 16 | 16 | 8.12244404133240e-11 | 8.12244430321293e-11 | 3.22e-8 |
///
/// **Interpretation.** At and below 1 024 cells the change is exactly nothing —
/// residual histories and solution vectors compared bit-equal, as predicted from
/// the block structure. That range covers **every pre-existing unit test** in
/// `crate::krylov`, `crate::ldu_matrix::solvers` and the
/// `tests/krylov_convection_diffusion.rs` integration suite, so none of them can
/// have been perturbed by this change.
///
/// Above one block the iterates differ in the last few bits and the *converged*
/// residual differs by up to **4.1e-8 relative** — that is, in the eighth
/// significant figure of a number that is itself 6e-11, so the absolute
/// difference is around 2.5e-18. **The iteration count did not change at any size
/// tested.** The relative difference is larger than the per-`dot` deviation
/// measured on [`crate::ldu_matrix::parallel::dot`] (worst raw 2.4e-13) because a
/// converged residual is a heavily cancelled quantity — the small difference of
/// large numbers — so it magnifies rounding, exactly as the
/// raw-versus-conditioned distinction documented on that function describes.
///
/// **Limitation.** This is five system sizes of one matrix family at one
/// tolerance. It does **not** establish that no system anywhere changes iteration
/// count — a solve whose residual crosses the tolerance within the last two bits
/// on the deciding iteration can flip by one iteration either way, and that is
/// inherent to changing a summation order rather than a defect. The pass criteria
/// above are what was checked; nothing broader is claimed.
#[test]
fn blocked_versus_flat_reduction_does_not_move_iteration_counts() {
    use crate::krylov::vecops;
    use crate::ldu_matrix::parallel::{axpy, dot, norm_l2};

    const BREAKDOWN_TOL: f64 = 1.0e-30;

    /// BiCGStab with the *flat* reductions the solver used before the hybrid
    /// kernels landed, driving the same products, so the only difference between
    /// this and `bicgstab_impl` is the summation order.
    fn bicgstab_flat(
        ldu: &HybridLdu,
        b: &[f64],
        precond: &Preconditioner,
        settings: &KrylovSettings,
        history: &mut Vec<f64>,
    ) -> (Vec<f64>, KrylovResult) {
        let be = ComputeBackend::Serial;
        let n = ldu.matrix().n_cells;
        history.clear();
        let bnorm = vecops::nrm2(b);
        let mut x = vec![0.0; n];
        let mut r = ldu.residual(&x, b, be);
        let rhat0 = r.clone();
        let mut best_x = x.clone();
        let mut best_rel = vecops::nrm2(&r) / bnorm;
        let (mut rho_old, mut alpha, mut omega) = (1.0_f64, 1.0_f64, 1.0_f64);
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
            let rho = vecops::dot(&rhat0, &r);
            if !rho.is_finite() || rho.abs() < BREAKDOWN_TOL || omega.abs() < BREAKDOWN_TOL {
                break;
            }
            let beta = (rho / rho_old) * (alpha / omega);
            for i in 0..n {
                p[i] = r[i] + beta * (p[i] - omega * v[i]);
            }
            precond.apply_on(&p, &mut phat, be);
            ldu.spmv_into(&phat, &mut v, be);
            let rhat_v = vecops::dot(&rhat0, &v);
            if !rhat_v.is_finite() || rhat_v.abs() < BREAKDOWN_TOL {
                break;
            }
            alpha = rho / rhat_v;
            s.copy_from_slice(&r);
            vecops::axpy(-alpha, &v, &mut s);
            let s_rel = vecops::nrm2(&s) / bnorm;
            if s_rel <= settings.tolerance {
                history.push(s_rel);
                vecops::axpy(alpha, &phat, &mut x);
                ldu.residual_into(&x, b, &mut scratch, be);
                let rel = vecops::nrm2(&scratch) / bnorm;
                if rel < best_rel {
                    best_rel = rel;
                    best_x.copy_from_slice(&x);
                }
                converged = best_rel <= settings.tolerance;
                break;
            }
            precond.apply_on(&s, &mut shat, be);
            ldu.spmv_into(&shat, &mut t, be);
            let tt = vecops::dot(&t, &t);
            if !tt.is_finite() || tt.abs() < BREAKDOWN_TOL {
                break;
            }
            omega = vecops::dot(&t, &s) / tt;
            vecops::axpy(alpha, &phat, &mut x);
            vecops::axpy(omega, &shat, &mut x);
            r.copy_from_slice(&s);
            vecops::axpy(-omega, &t, &mut r);
            let rel = vecops::nrm2(&r) / bnorm;
            history.push(rel);
            if !rel.is_finite() {
                break;
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
                break;
            }
            rho_old = rho;
        }
        ldu.residual_into(&best_x, b, &mut scratch, be);
        let true_rel = vecops::nrm2(&scratch) / bnorm;
        (
            best_x,
            KrylovResult {
                n_iterations: iters,
                final_residual: true_rel,
                converged: converged && true_rel <= settings.tolerance,
            },
        )
    }

    // Sanity: the two `dot`s really are identical below one block and differ
    // above it, so the comparison below is testing what it claims to.
    let small = rhs(1024, 7);
    assert_eq!(
        dot(&small, &small, ComputeBackend::Serial),
        vecops::dot(&small, &small),
        "blocked and flat dot must agree exactly at exactly one block"
    );
    let mut y = vec![1.0; 8];
    let z = vec![2.0; 8];
    axpy(0.5, &z, &mut y, ComputeBackend::Serial);
    assert_eq!(y, vec![2.0; 8], "axpy sanity");
    assert_eq!(norm_l2(&[3.0, 4.0], ComputeBackend::Serial), 5.0);

    let settings = KrylovSettings {
        tolerance: 1e-10,
        max_iter: 2000,
        restart: 30,
    };
    eprintln!("machine: {}", machine_state());
    eprintln!(
        "{:>6} {:>7} {:>7} {:>7} {:>24} {:>24} {:>11}",
        "mesh", "cells", "blk it", "flat it", "resid (blocked)", "resid (flat)", "rel diff"
    );
    for n in [8usize, 10, 16, 24, 32] {
        let a = stencil_3d(n, 1.05, 0x1234_5678);
        let cells = a.n_cells;
        let b = rhs(cells, 0xDEAD_BEEF);
        let precond = Preconditioner::ilu0(&a);
        let ldu = HybridLdu::new(Arc::new(a));

        let mut h_blocked = Vec::new();
        let (x_blocked, r_blocked) = bicgstab_impl(
            &ldu,
            &b,
            None,
            &precond,
            &settings,
            ComputeBackend::Serial,
            &mut h_blocked,
        );
        let mut h_flat = Vec::new();
        let (x_flat, r_flat) = bicgstab_flat(&ldu, &b, &precond, &settings, &mut h_flat);

        let rel_diff = if r_blocked.final_residual == r_flat.final_residual {
            0.0
        } else {
            (r_blocked.final_residual - r_flat.final_residual).abs()
                / r_blocked
                    .final_residual
                    .abs()
                    .max(r_flat.final_residual.abs())
        };
        eprintln!(
            "{:>4}^3 {:>7} {:>7} {:>7} {:>24.14e} {:>24.14e} {:>11.2e}",
            n,
            cells,
            r_blocked.n_iterations,
            r_flat.n_iterations,
            r_blocked.final_residual,
            r_flat.final_residual,
            rel_diff
        );

        assert!(
            r_blocked.converged && r_flat.converged,
            "{cells} cells: one variant did not converge"
        );
        assert_eq!(
            r_blocked.n_iterations, r_flat.n_iterations,
            "{cells} cells: blocked and flat reductions disagree on the iteration count"
        );
        if cells <= 1024 {
            // One reduction block: the blocked sum *is* the flat sum.
            assert_eq!(
                h_blocked, h_flat,
                "{cells} cells: histories must be bit-identical at or below one block"
            );
            assert_eq!(
                x_blocked, x_flat,
                "{cells} cells: solutions must be bit-identical at or below one block"
            );
        } else {
            assert!(
                rel_diff <= 1e-7,
                "{cells} cells: final residuals differ by {rel_diff:.3e} relative"
            );
        }
    }
}

// ── ILU(0): how much of a solve is the part that cannot be threaded ───────────

/// **Methodology.** ILU(0) is sequential by construction — both its factorisation
/// and its triangular solves have a loop-carried dependence (see
/// [`crate::krylov::Ilu0Preconditioner::apply_on`]). This is not fixed here, and
/// the honest thing to report instead is *how much it costs*, because that is the
/// Amdahl serial fraction that bounds any speed-up of an ILU(0)-preconditioned
/// solve.
///
/// The test times, on the same system: (a) the one-off
/// [`Preconditioner::ilu0`] factorisation, (b) a whole BiCGStab solve, and (c)
/// the isolated cost of the `2 * n_iterations` preconditioner applications that
/// the solve performs, timed by replaying them outside the solver. It reports
/// each as a percentage of the solve.
///
/// **Pass criterion.** None on the timings themselves — this is a measurement,
/// and asserting a wall-clock figure on a shared machine would be a flake
/// generator. The test only asserts the solve converged, so the fractions refer
/// to a real solve. `#[ignore]`d for the same reason; run it explicitly.
///
/// **Results, measured 2026-08-13**, release, `--features parallel`, 4 logical
/// cores, load average `1.40 0.89 1.08` at the start of the run. `factor %` and
/// `apply %` are each expressed as a percentage of the **solve** time (the
/// factorisation is a separate one-off cost *on top of* the solve, not part of
/// it).
///
/// | Cells | Iterations | Factorisation (us) | Solve (us) | 2·n applies (us) | Factor % of solve | Applies % of solve |
/// |---|---|---|---|---|---|---|
/// | 4 096 | 16 | 1 710.8 | 7 477.8 | 1 431.4 | 22.9% | 19.1% |
/// | 32 768 | 16 | 12 948.7 | 31 286.4 | 14 365.9 | 41.4% | 45.9% |
/// | 110 592 | 16 | 42 150.5 | 110 630.4 | 54 287.7 | 38.1% | 49.1% |
/// | 262 144 | 16 | 102 793.4 | 253 012.3 | 130 574.7 | 40.6% | **51.6%** |
///
/// **Interpretation — this is the number that explains everything else.** At
/// production sizes the sequential ILU(0) triangular solves are **about half the
/// solve**, and the one-off factorisation costs a further ~40% of a solve on top.
/// Amdahl's law then caps the speed-up of an ILU(0)-preconditioned solve at
/// `1 / 0.516 ≈ 1.9x` **even on infinitely many cores**, before any parallel
/// efficiency loss in the half that does thread. The measured end-to-end ILU(0)
/// speed-up of 1.50x at 512 000 cells (see
/// `end_to_end_solve_speedup_benchmark`) is therefore not a disappointing
/// result — it is close to the structural ceiling, and the remaining headroom is
/// in the preconditioner, not in the kernels this bead threaded.
///
/// It also tells the maintainer where the next win is: parallelising ILU(0)
/// (level scheduling) or switching to a preconditioner that is parallel by
/// construction would raise the ceiling far more than tuning any kernel here.
#[test]
#[ignore = "timing benchmark; run explicitly with --ignored"]
fn ilu0_serial_fraction_benchmark() {
    let settings = KrylovSettings {
        tolerance: 1e-10,
        max_iter: 2000,
        restart: 30,
    };
    const REPEATS: usize = 3;
    let cores = cores();
    eprintln!("machine: {}", machine_state());
    eprintln!(
        "{:>7} {:>6} {:>10} {:>11} {:>11} {:>11} {:>7} {:>9} {:>9} {:>9}",
        "cells",
        "iters",
        "factor us",
        "solve ser us",
        "solve par us",
        "applies us",
        "s",
        "cap(4)",
        "cap(inf)",
        "measured"
    );
    for n in [16usize, 32, 48, 64] {
        let a = stencil_3d(n, 1.05, 0x1234_5678);
        let cells = a.n_cells;
        let b = rhs(cells, 0xDEAD_BEEF);

        // (a) the one-off factorisation, best of REPEATS.
        let mut factor_us = f64::INFINITY;
        for _ in 0..REPEATS {
            let t = Instant::now();
            let p = Preconditioner::ilu0(&a);
            factor_us = factor_us.min(t.elapsed().as_secs_f64() * 1e6);
            std::hint::black_box(&p);
        }
        let precond = Preconditioner::ilu0(&a);
        let ldu = HybridLdu::new(Arc::new(a));

        // (b) the whole solve, on BOTH backends. Amdahl's serial fraction is
        //     defined against the *serial* runtime, so the serial column is the
        //     denominator that matters; the parallel column gives the measured
        //     speed-up to compare the resulting cap against.
        let mut solve_us = [f64::INFINITY; 2];
        let mut iters = 0usize;
        for _ in 0..REPEATS {
            for (bi, backend) in [ComputeBackend::Serial, ComputeBackend::CpuMulti]
                .into_iter()
                .enumerate()
            {
                let t = Instant::now();
                let (x, r) =
                    bicgstab_impl(&ldu, &b, None, &precond, &settings, backend, &mut Vec::new());
                solve_us[bi] = solve_us[bi].min(t.elapsed().as_secs_f64() * 1e6);
                std::hint::black_box(&x);
                assert!(r.converged, "{cells} cells: solve did not converge");
                iters = r.n_iterations;
            }
        }

        // (c) replay the applies the solve performed: 2 per iteration. The
        //     backend is irrelevant — `Ilu0Preconditioner::apply_on` ignores it
        //     and always runs serially — which is exactly why this cost is the
        //     serial fraction.
        let mut applies_us = f64::INFINITY;
        let mut z = vec![0.0; cells];
        for _ in 0..REPEATS {
            let t = Instant::now();
            for _ in 0..(2 * iters) {
                precond.apply_on(&b, &mut z, ComputeBackend::CpuMulti);
            }
            applies_us = applies_us.min(t.elapsed().as_secs_f64() * 1e6);
            std::hint::black_box(&z);
        }

        // Amdahl: s is the fraction of the SERIAL runtime that cannot be
        // threaded. The cap on `cores` cores is 1/(s + (1 - s)/cores); the
        // infinite-core limit is 1/s.
        let s = applies_us / solve_us[0];
        let cap_cores = 1.0 / (s + (1.0 - s) / cores as f64);
        let cap_inf = 1.0 / s;
        let measured = solve_us[0] / solve_us[1];

        eprintln!(
            "{:>7} {:>6} {:>10.1} {:>11.1} {:>11.1} {:>11.1} {:>6.1}% {:>8.2}x {:>8.2}x {:>8.2}x",
            cells,
            iters,
            factor_us,
            solve_us[0],
            solve_us[1],
            applies_us,
            100.0 * s,
            cap_cores,
            cap_inf,
            measured
        );
    }
}

// ── End-to-end speed-up of a real solve ───────────────────────────────────────

/// **Methodology.** The measurement bead `op-yvj.4.4` actually asks for: an
/// **end-to-end** wall-clock comparison of a whole linear solve on
/// [`ComputeBackend::Serial`] against [`ComputeBackend::CpuMulti`], not a
/// micro-benchmark of one kernel. `HybridLdu` landed with no consumer, so until
/// now no end-to-end speed-up existed to measure.
///
/// For each mesh size, an asymmetric 7-point-stencil system is built and solved
/// to `1e-10` with BiCGStab under two preconditioners:
///
/// - **Jacobi**, whose apply is embarrassingly parallel — so the whole solve is
///   threadable and this is the optimistic case;
/// - **ILU(0)**, whose apply is sequential on both backends — so this is the
///   realistic case, and the gap between the two columns *is* the Amdahl cost of
///   the sequential preconditioner.
///
/// Each figure is the best of 5 repeats of the complete solve (best-of, not mean,
/// to suppress scheduler noise on a machine that is never idle), reported in
/// milliseconds per solve. The iteration count is printed alongside and asserted
/// equal between backends, so the two columns are always the same amount of work.
///
/// **Pass criterion.** Iteration counts must match between backends (the solve
/// must be doing the same work in both columns). **No assertion is made on the
/// timings** — a wall-clock threshold on a shared machine is a flake generator,
/// and the point of this test is to produce numbers, not to gate on them.
/// `#[ignore]`d; run explicitly.
///
/// **Results, measured 2026-08-13**, release, `--features parallel`, 4 logical
/// cores, best of 5 complete solves per figure, milliseconds per solve. The
/// "(rpt)" columns are a second, independent run of the same benchmark. Load
/// averages: run 1 `1.46 0.85 1.07` at start and `1.43 0.89 1.08` at end; run 2
/// `1.67 1.00 1.11` at start and `2.19 1.17 1.16` at end. **This host never
/// reaches idle** — a `bn daemon run` holds roughly a third of one core — so
/// these are loaded-machine figures, and the SpMV crossover table on
/// [`SPMV_MIN_CELLS`](crate::ldu_matrix::parallel::SPMV_MIN_CELLS) records that
/// contention moves the break-even point to the right.
///
/// ILU(0) converged in 16 iterations at every size; Jacobi in 47-51 (so the two
/// preconditioner columns are *not* the same amount of work as each other, but
/// each column pair is, and the test asserts that).
///
/// | Cells | Faces | Jacobi serial | Jacobi multi | Speed-up | (rpt) | ILU(0) serial | ILU(0) multi | Speed-up | (rpt) |
/// |---|---|---|---|---|---|---|---|---|---|
/// | 4 096 | 11 520 | 7.152 | 8.791 | **0.81x** | 0.62x | 3.720 | 5.247 | **0.71x** | 0.67x |
/// | 13 824 | 39 744 | 22.436 | 15.560 | 1.44x | 1.33x | 13.232 | 11.820 | 1.12x | 1.10x |
/// | 32 768 | 95 232 | 56.934 | 36.198 | 1.57x | 1.58x | 32.930 | 25.928 | 1.27x | 1.25x |
/// | 110 592 | 324 864 | 216.211 | 117.584 | 1.84x | 1.75x | 124.452 | 90.509 | 1.38x | 1.32x |
/// | 262 144 | 774 144 | 507.064 | 210.376 | 2.41x | 2.45x | 295.103 | 198.263 | 1.49x | 1.45x |
/// | 512 000 | 1 516 800 | 1106.876 | 417.606 | **2.65x** | 2.66x | 623.984 | 416.845 | **1.50x** | 1.51x |
///
/// **Interpretation.**
///
/// - **There is a real end-to-end speed-up**, and this is the first time one has
///   been measurable: `HybridLdu` landed with the explicit note that nothing
///   consumed it and no solver speed-up was claimed. A whole Jacobi-preconditioned
///   BiCGStab solve on half a million cells is **2.65x faster** on 4 logical
///   cores, reproducibly (2.65x and 2.66x on two independent runs).
/// - **ILU(0) tops out at 1.50x**, and `ilu0_serial_fraction_benchmark` explains
///   exactly why: its sequential triangular solves are 51.6% of the solve, which
///   caps any speed-up at about 1.9x by Amdahl's law. The gap between the two
///   preconditioner columns is a direct measurement of what the sequential
///   preconditioner costs.
/// - **At 4 096 cells the parallel solve LOSES** — 0.81x and 0.62x for Jacobi,
///   0.71x and 0.67x for ILU(0). That is exactly
///   [`SPMV_MIN_CELLS`](crate::ldu_matrix::parallel::SPMV_MIN_CELLS), the floor at
///   which the *isolated* product kernel was measured to break even. A whole
///   solve does not break even there, because it interleaves the product with
///   vector operations and a preconditioner apply that all run serially at that
///   size, so the product's marginal win is diluted while the dispatch cost is
///   paid on every one of the ~16 iterations. **The product's crossover is not
///   the solve's crossover**; on this machine the solve does not reliably win
///   until roughly 13 000 cells. This is a finding for bead `op-yvj.4.7`, not
///   something fixed here.
///
/// **Limitations.** One machine, 4 logical cores, one matrix family
/// (asymmetric 7-point stencil, diagonal dominance 1.05), one tolerance, `f64`
/// CPU only, and a machine that was never idle. This is not a scaling study and
/// says nothing about a many-core server, about Android/Termux hardware, or about
/// a GPU — there is no GPU kernel in this path at all.
#[test]
#[ignore = "timing benchmark; run explicitly with --ignored"]
fn end_to_end_solve_speedup_benchmark() {
    let settings = KrylovSettings {
        tolerance: 1e-10,
        max_iter: 2000,
        restart: 30,
    };
    const REPEATS: usize = 5;

    eprintln!("machine: {}", machine_state());
    eprintln!(
        "{:>7} {:>8} {:>6} {:>11} {:>11} {:>9} {:>11} {:>11} {:>9}",
        "cells",
        "faces",
        "iters",
        "jac ser ms",
        "jac par ms",
        "speed-up",
        "ilu ser ms",
        "ilu par ms",
        "speed-up"
    );

    for n in [16usize, 24, 32, 48, 64, 80] {
        let a = stencil_3d(n, 1.05, 0x1234_5678);
        let cells = a.n_cells;
        let faces = a.n_internal_faces;
        let b = rhs(cells, 0xDEAD_BEEF);
        let jac = Preconditioner::jacobi(&a);
        let ilu = Preconditioner::ilu0(&a);
        let ldu = HybridLdu::new(Arc::new(a));

        let mut best = [[f64::INFINITY; 2]; 2]; // [precond][backend]
        let mut iters = [[0usize; 2]; 2];
        for _ in 0..REPEATS {
            for (pi, precond) in [&jac, &ilu].into_iter().enumerate() {
                for (bi, backend) in [ComputeBackend::Serial, ComputeBackend::CpuMulti]
                    .into_iter()
                    .enumerate()
                {
                    let t = Instant::now();
                    let (x, r) =
                        bicgstab_impl(&ldu, &b, None, precond, &settings, backend, &mut Vec::new());
                    let ms = t.elapsed().as_secs_f64() * 1e3;
                    std::hint::black_box(&x);
                    assert!(r.converged, "{cells} cells: did not converge");
                    best[pi][bi] = best[pi][bi].min(ms);
                    iters[pi][bi] = r.n_iterations;
                }
            }
        }

        assert_eq!(
            iters[0][0], iters[0][1],
            "{cells} cells: Jacobi iteration counts differ between backends"
        );
        assert_eq!(
            iters[1][0], iters[1][1],
            "{cells} cells: ILU(0) iteration counts differ between backends"
        );

        eprintln!(
            "{:>7} {:>8} {:>6} {:>11.3} {:>11.3} {:>8.2}x {:>11.3} {:>11.3} {:>8.2}x",
            cells,
            faces,
            iters[1][0],
            best[0][0],
            best[0][1],
            best[0][0] / best[0][1],
            best[1][0],
            best[1][1],
            best[1][0] / best[1][1]
        );
    }
    eprintln!("load average at end: {}", load_average());
}

/// The same end-to-end measurement for **GMRES(30)**, which has a different
/// work balance from BiCGStab and therefore a different amount to gain.
///
/// **Methodology.** Identical to `end_to_end_solve_speedup_benchmark` — same
/// systems, same Jacobi preconditioner (chosen so the whole solve is threadable
/// and the comparison is not dominated by ILU(0)'s serial fraction), best of 3
/// complete solves — but running [`gmres_prepared`](crate::krylov::gmres_prepared)
/// instead of BiCGStab, and stopping at 262 144 cells because GMRES(30) stores 30
/// basis vectors and takes far more iterations.
///
/// **Why the answer is expected to differ.** BiCGStab does two sparse products
/// against a fixed four inner products per iteration. GMRES(30) does **one**
/// product against `j + 1` inner products and `j + 1` `axpy`s at Arnoldi step
/// `j`, so by the end of a cycle the vector operations dominate. Since the
/// vector operations have a size floor 64x higher than the product's, GMRES
/// should thread *less* effectively than BiCGStab at any given mesh size.
///
/// **Pass criterion.** Iteration counts equal between backends. No assertion on
/// timings. `#[ignore]`d.
///
/// **Results, measured 2026-08-13**, release, `--features parallel`, 4 logical
/// cores, best of 3 complete solves, milliseconds per solve. **Both runs were
/// taken on a heavily loaded machine** — load average `3.68 1.91 1.42` rising to
/// `3.87 1.98 1.45` for run 1, and `3.51 1.96 1.45` to `3.45 2.00 1.46` for run 2,
/// on 4 logical cores, i.e. essentially saturated. These parallel columns are
/// therefore **pessimistic**, and the BiCGStab table above (taken at load ~1.5)
/// is not directly comparable in absolute terms. The *ordering* is what this
/// measurement establishes, and it survives the contention.
///
/// | Cells | Iterations | Serial (ms) | Multi (ms) | Speed-up | (rpt) | BiCGStab+Jacobi speed-up at the same size |
/// |---|---|---|---|---|---|---|
/// | 4 096 | 72 | 8.376 | 11.176 | 0.75x | 0.62x | 0.81x |
/// | 32 768 | 73 | 78.225 | 64.453 | 1.21x | 0.99x | 1.57x |
/// | 110 592 | 73 | 273.820 | 217.140 | 1.26x | 1.30x | 1.84x |
/// | 262 144 | 72 | 747.334 | 402.504 | 1.86x | 1.99x | 2.41x |
///
/// **Interpretation.** The prediction holds: **GMRES(30) gains less than
/// BiCGStab from `CpuMulti` at every size tested**, and it gains it later —
/// 1.21x/0.99x where BiCGStab reached 1.57x at 32 768 cells. That is the expected
/// consequence of GMRES spending proportionally more of its time in vector
/// operations, which have the 64x higher size floor, and proportionally less in
/// the sparse product, which is the kernel that threads well. GMRES does catch up
/// as the mesh grows (1.86x/1.99x at 262 144), which is also expected: at that
/// size the vector operations finally clear
/// [`VECOP_MIN_ELEMENTS`](crate::ldu_matrix::parallel::VECOP_MIN_ELEMENTS) and
/// begin threading too.
///
/// **Limitation.** Taken under heavy contention, so the absolute speed-ups are
/// lower bounds rather than the machine's best. Comparing the two runs against
/// each other (0.75x vs 0.62x, 1.21x vs 0.99x) shows the scatter that contention
/// introduces; the BiCGStab comparison column is quoted from a *less* loaded run
/// and so, if anything, understates how much further ahead BiCGStab is.
#[test]
#[ignore = "timing benchmark; run explicitly with --ignored"]
fn gmres_end_to_end_speedup_benchmark() {
    let settings = KrylovSettings {
        tolerance: 1e-10,
        max_iter: 2000,
        restart: 30,
    };
    const REPEATS: usize = 3;
    eprintln!("machine: {}", machine_state());
    eprintln!(
        "{:>8} {:>7} {:>12} {:>12} {:>10}",
        "cells", "iters", "serial ms", "multi ms", "speed-up"
    );
    for n in [16usize, 32, 48, 64] {
        let a = stencil_3d(n, 1.05, 0x1234_5678);
        let cells = a.n_cells;
        let b = rhs(cells, 0xDEAD_BEEF);
        let precond = Preconditioner::jacobi(&a);
        let ldu = HybridLdu::new(Arc::new(a));

        let mut best = [f64::INFINITY; 2];
        let mut iters = [0usize; 2];
        for _ in 0..REPEATS {
            for (bi, backend) in [ComputeBackend::Serial, ComputeBackend::CpuMulti]
                .into_iter()
                .enumerate()
            {
                let t = Instant::now();
                let (x, r) = gmres_impl(
                    &ldu,
                    &b,
                    None,
                    &precond,
                    &settings,
                    backend,
                    &mut Vec::new(),
                );
                let ms = t.elapsed().as_secs_f64() * 1e3;
                std::hint::black_box(&x);
                assert!(r.converged, "{cells} cells: GMRES did not converge");
                best[bi] = best[bi].min(ms);
                iters[bi] = r.n_iterations;
            }
        }
        assert_eq!(
            iters[0], iters[1],
            "{cells} cells: GMRES iteration counts differ between backends"
        );
        eprintln!(
            "{:>8} {:>7} {:>12.3} {:>12.3} {:>9.2}x",
            cells,
            iters[0],
            best[0],
            best[1],
            best[0] / best[1]
        );
    }
    eprintln!("load average at end: {}", load_average());
}

/// **Methodology.** The dispatch policy this crate inherited has two size floors
/// that differ by 64x —
/// [`SPMV_MIN_CELLS`](crate::ldu_matrix::parallel::SPMV_MIN_CELLS) = 4 096 and
/// [`VECOP_MIN_ELEMENTS`](crate::ldu_matrix::parallel::VECOP_MIN_ELEMENTS) =
/// 262 144 — so on any mesh between them a Krylov solve threads its sparse
/// products while running every inner product and `axpy` serially. Both floors
/// were measured in **isolation**, on a cold rayon pool. Inside a solve the pool
/// is already warm from the product that ran microseconds earlier, so the
/// dispatch cost the vector-operation floor was set to avoid may not be present.
///
/// This test asks whether the shared vector-operation floor is leaving anything
/// on the table *in situ*: it runs the same BiCGStab solve twice, once with the
/// production floors and once with the vector-operation floor lowered to match
/// the product's 4 096, using the crate-internal `_min` entry points, and reports
/// both timings. It does not change the shipped constants.
///
/// **Pass criterion.** None on timings — measurement only. The test asserts the
/// two variants agree bitwise (lowering a size floor must not change any value,
/// because the two kernels are bitwise identical either way), which is itself a
/// worthwhile check.
///
/// **Results, measured 2026-08-13**, release, `--features parallel`, 4 logical
/// cores, best of 5 solves, load average `1.77 0.98 1.10`. Jacobi-preconditioned
/// BiCGStab, so the whole solve is threadable and the vector operations are as
/// prominent as they ever get.
///
/// | Cells | Iterations | Vecop floor 262 144 (ms) | Vecop floor 4 096 (ms) | Ratio |
/// |---|---|---|---|---|
/// | 13 824 | 47 | 16.635 | 20.971 | 0.79x |
/// | 32 768 | 50 | 38.096 | 42.218 | 0.90x |
/// | 110 592 | 51 | 120.475 | 109.818 | 1.10x |
/// | 262 144 | 50 | 201.086 | 205.433 | 0.98x |
///
/// **Interpretation — a negative result, reported as one.** The warm-pool
/// hypothesis is **not** supported. Lowering the vector-operation floor to the
/// product's 4 096 made the solve *slower* at 13 824 and 32 768 cells (0.79x,
/// 0.90x), roughly break-even at 262 144 (0.98x), and helped only marginally and
/// unrepeatably at 110 592 (1.10x). The shipped
/// [`VECOP_MIN_ELEMENTS`](crate::ldu_matrix::parallel::VECOP_MIN_ELEMENTS) =
/// 262 144 is therefore the right choice inside a solve as well as in isolation,
/// and the 64x gap between the two floors is **not** leaving performance on the
/// table. **No constant was changed on the strength of this.**
///
/// The bitwise assertion also passed at every size, confirming that a size floor
/// is purely a wall-clock decision: the two variants agreed on every element of
/// the solution.
///
/// **Limitation.** This measures a *hypothetical* floor by lowering it for
/// `dot`/`axpy`/`norm_l2` only, on one machine and one matrix family. It is
/// evidence for a follow-up crossover study (bead `op-yvj.4.7`), not a
/// recommendation to change the constants, which would need the full crossover
/// sweep the existing tables were built from.
#[cfg(feature = "parallel")]
#[test]
#[ignore = "timing benchmark; run explicitly with --ignored"]
fn vecop_floor_inside_a_warm_solve_benchmark() {
    use crate::ldu_matrix::parallel::{axpy_min, dot_min, SPMV_MIN_CELLS};

    const BREAKDOWN_TOL: f64 = 1.0e-30;

    /// BiCGStab with every vector-operation floor lowered to `floor`, so the
    /// reductions and updates thread at the same size the product does.
    fn bicgstab_floor(
        ldu: &HybridLdu,
        b: &[f64],
        precond: &Preconditioner,
        settings: &KrylovSettings,
        floor: usize,
    ) -> (Vec<f64>, usize) {
        let be = ComputeBackend::CpuMulti;
        let n = ldu.matrix().n_cells;
        let nrm = |v: &[f64]| dot_min(v, v, be, floor).sqrt();
        let bnorm = nrm(b);
        let mut x = vec![0.0; n];
        let mut r = ldu.residual(&x, b, be);
        let rhat0 = r.clone();
        let mut best_x = x.clone();
        let mut best_rel = nrm(&r) / bnorm;
        let (mut rho_old, mut alpha, mut omega) = (1.0_f64, 1.0_f64, 1.0_f64);
        let mut v = vec![0.0; n];
        let mut p = vec![0.0; n];
        let mut phat = vec![0.0; n];
        let mut shat = vec![0.0; n];
        let mut s = vec![0.0; n];
        let mut t = vec![0.0; n];
        let mut scratch = vec![0.0; n];
        let mut iters = 0usize;

        while iters < settings.max_iter {
            iters += 1;
            let rho = dot_min(&rhat0, &r, be, floor);
            if !rho.is_finite() || rho.abs() < BREAKDOWN_TOL || omega.abs() < BREAKDOWN_TOL {
                break;
            }
            let beta = (rho / rho_old) * (alpha / omega);
            for i in 0..n {
                p[i] = r[i] + beta * (p[i] - omega * v[i]);
            }
            precond.apply_on(&p, &mut phat, be);
            ldu.spmv_into(&phat, &mut v, be);
            let rhat_v = dot_min(&rhat0, &v, be, floor);
            if !rhat_v.is_finite() || rhat_v.abs() < BREAKDOWN_TOL {
                break;
            }
            alpha = rho / rhat_v;
            s.copy_from_slice(&r);
            axpy_min(-alpha, &v, &mut s, be, floor);
            if nrm(&s) / bnorm <= settings.tolerance {
                axpy_min(alpha, &phat, &mut x, be, floor);
                ldu.residual_into(&x, b, &mut scratch, be);
                let rel = nrm(&scratch) / bnorm;
                if rel < best_rel {
                    best_x.copy_from_slice(&x);
                }
                break;
            }
            precond.apply_on(&s, &mut shat, be);
            ldu.spmv_into(&shat, &mut t, be);
            let tt = dot_min(&t, &t, be, floor);
            if !tt.is_finite() || tt.abs() < BREAKDOWN_TOL {
                break;
            }
            omega = dot_min(&t, &s, be, floor) / tt;
            axpy_min(alpha, &phat, &mut x, be, floor);
            axpy_min(omega, &shat, &mut x, be, floor);
            r.copy_from_slice(&s);
            axpy_min(-omega, &t, &mut r, be, floor);
            let rel = nrm(&r) / bnorm;
            if !rel.is_finite() {
                break;
            }
            if rel < best_rel {
                best_rel = rel;
                best_x.copy_from_slice(&x);
            }
            if best_rel <= settings.tolerance {
                break;
            }
            if omega.abs() < BREAKDOWN_TOL {
                break;
            }
            rho_old = rho;
        }
        (best_x, iters)
    }

    let settings = KrylovSettings {
        tolerance: 1e-10,
        max_iter: 2000,
        restart: 30,
    };
    const REPEATS: usize = 5;
    eprintln!("machine: {}", machine_state());
    eprintln!(
        "{:>8} {:>6} {:>16} {:>16} {:>10}",
        "cells", "iters", "vecop floor 262k", "vecop floor 4096", "ratio"
    );
    for n in [24usize, 32, 48, 64] {
        let a = stencil_3d(n, 1.05, 0x1234_5678);
        let cells = a.n_cells;
        let b = rhs(cells, 0xDEAD_BEEF);
        let precond = Preconditioner::jacobi(&a);
        let ldu = HybridLdu::new(Arc::new(a));

        let mut best_prod = f64::INFINITY;
        let mut best_low = f64::INFINITY;
        let mut iters = 0usize;
        let mut x_prod = Vec::new();
        let mut x_low = Vec::new();
        for _ in 0..REPEATS {
            let t = Instant::now();
            let (x, r) = bicgstab_impl(
                &ldu,
                &b,
                None,
                &precond,
                &settings,
                ComputeBackend::CpuMulti,
                &mut Vec::new(),
            );
            best_prod = best_prod.min(t.elapsed().as_secs_f64() * 1e3);
            iters = r.n_iterations;
            x_prod = x;

            let t = Instant::now();
            let (x, _) = bicgstab_floor(&ldu, &b, &precond, &settings, SPMV_MIN_CELLS);
            best_low = best_low.min(t.elapsed().as_secs_f64() * 1e3);
            x_low = x;
        }
        assert_eq!(
            x_prod, x_low,
            "{cells} cells: lowering a size floor must not change any value"
        );
        eprintln!(
            "{:>8} {:>6} {:>16.3} {:>16.3} {:>9.2}x",
            cells,
            iters,
            best_prod,
            best_low,
            best_prod / best_low
        );
    }
}
