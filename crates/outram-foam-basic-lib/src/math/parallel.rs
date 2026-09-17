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

//! Batched scalar root finding on the hybrid execution backend — solve `N`
//! independent `f(x) = 0` problems at once, serially or across CPU cores.
//!
//! # The shape of the problem
//!
//! A finite-volume solver almost never wants *one* root. It wants one root per
//! cell: given a cell-wise target enthalpy `h_target[c]`, find the temperature
//! `T[c]` satisfying `h(T[c]) - h_target[c] = 0`, for every cell in the mesh.
//! Those `N` problems are completely independent of one another — no cell's
//! iteration reads another cell's iterate — which is what makes the batch worth
//! threading at all.
//!
//! This module therefore provides **batched** entry points only. A single scalar
//! root solve should stay serial and inline in its caller: one root is a few
//! microseconds of work and threading it is a straight loss.
//!
//! | Operation | Derivative needed? | Entry point |
//! |---|---|---|
//! | bisection or Brent over a bracket | no | [`solve_bracketed_batch`] |
//! | safeguarded Newton over a bracket | yes | [`solve_newton_batch`] |
//! | closed-form roots of `a x + b` | — | [`linear_roots_batch`] |
//! | closed-form roots of `a x^2 + b x + c` | — | [`quadratic_roots_batch`] |
//! | closed-form roots of `a x^3 + b x^2 + c x + d` | — | [`cubic_roots_batch`] |
//!
//! # Hybrid means dispatch, not two APIs
//!
//! Every entry point takes a [`ComputeBackend`] parameter. There is no
//! `solve_batch_parallel()` sibling beside `solve_batch()`. With the `parallel`
//! feature off, [`ComputeBackend::CpuMulti`] resolves down to
//! [`ComputeBackend::Serial`] through [`ComputeBackend::resolve`] and the answer
//! is unchanged — bit for bit, not merely close. There is no `Gpu` kernel here
//! yet, so a `Gpu` request degrades to the best available CPU path.
//!
//! # Determinism — bitwise identical, and why that is stronger here
//!
//! **Every kernel in this module returns bit-for-bit identical output on
//! [`ComputeBackend::Serial`] and [`ComputeBackend::CpuMulti`], at any thread
//! count, on every run** — provided the caller's residual closure is itself a
//! deterministic pure function of `(index, x)`.
//!
//! This is easier to achieve than it is for a reduction and the reason is worth
//! stating, because it is the thing most people get wrong about parallel root
//! finding. A parallel sum has to re-associate `a + b + c`, and floating-point
//! addition is not associative, so its last bits depend on the split. A batch of
//! root solves has **no cross-element arithmetic at all**: lane `i`'s answer is a
//! pure function of lane `i`'s bracket and closure evaluations. Splitting the
//! lanes across threads cannot perturb any of them. Both backends call the very
//! same `#[inline]` per-lane kernel; only the identity of the calling thread
//! differs.
//!
//! The one way a caller can break this is to supply a closure that is not a pure
//! function — one that reads a random number generator, or accumulates into
//! shared interior-mutable state, or depends on the calling thread. Such a
//! closure makes the batch non-reproducible no matter what this module does. The
//! `Sync` bound permits it; the documented contract forbids it.
//!
//! Verified by the `bitwise_*` tests in `parallel/tests.rs`, which compare
//! serial against `rayon` pools of 1, 2, 4 and 8 workers on a batch deliberately
//! built with wildly varying per-lane iteration counts.
//!
//! # Load imbalance — why there is no hand-rolled partition
//!
//! Iteration counts vary per lane, sometimes by an order of magnitude: a lane
//! whose bracket happens to straddle the root tightly converges in three
//! iterations, and its neighbour takes eighty. A static equal-split across `P`
//! threads therefore ends up waiting on whichever chunk drew the hard lanes.
//!
//! The parallel paths here use `rayon`'s adaptive splitting (`par_iter_mut`,
//! `into_par_iter`) with **no** `min_len` floor on the iterative solvers, so an
//! idle worker steals down to individual lanes when the work is skewed. That is
//! a deliberate choice, not an oversight: the whole reason to prefer
//! work-stealing here is the per-lane cost variance. The closed-form polynomial
//! kernels are the exception — every lane there costs the same handful of
//! floating-point operations, so they set a granularity floor of
//! [`POLY_BLOCK`] lanes to stop the splitter subdividing below a task that
//! cannot pay for itself. That floor affects wall clock only; it cannot change a
//! value, because each lane is independent.
//!
//! For scale, a 65 536-lane deliberately-imbalanced batch (half the lanes given
//! a 2e-6-wide bracket, half given `[0, 1e6]`) under Brent, measured 2026-08-12
//! on 4 logical cores by the `#[ignore]`d `root_batch_thread_scaling_benchmark`,
//! best of 7 samples, with a second independent run alongside:
//!
//! | Worker threads | Time | Speed-up | (repeat) | Bitwise vs serial |
//! |---|---|---|---|---|
//! | *serial reference* | 19322.52 us | 1.00x | 1.00x | — |
//! | 1 | 20510.52 us | 0.94x | 0.96x | identical |
//! | 2 | 10123.65 us | 1.91x | 1.88x | identical |
//! | 4 | 7075.89 us | 2.73x | 3.66x | identical |
//! | 8 | 7314.01 us | 2.64x | 3.57x | identical |
//!
//! The "identical" column is the determinism claim above measured rather than
//! argued, and it is asserted by the benchmark itself, not merely printed. The
//! one-worker row costs about 6% against plain serial, which is the price of
//! going through `rayon` at all. Unlike the memory-latency-bound sparse product
//! in [`crate::ldu_matrix::parallel`], eight workers on four cores buy nothing
//! further here — the expected signature of a compute-bound kernel that already
//! saturates the cores it has. **This is one machine, one batch and two runs; it
//! is not a scaling study**, and nothing here has been measured on Android
//! hardware or on a many-core server.
//!
//! # Non-convergence is reported, never swallowed
//!
//! A batch of 10 000 problems in which 3 fail must say so. This module reports
//! failure **per lane**, and makes the failure impossible to ignore by
//! construction:
//!
//! - Every lane carries a [`RootStatus`] and is reachable through
//!   [`RootBatch::solutions`].
//! - [`RootSolution::root`] returns `Option<f64>` — `Some(x)` **only** when that
//!   lane converged. There is no accessor that hands out a bare `f64` without
//!   the caller having seen the status; the diagnostic iterate is behind the
//!   deliberately-named [`RootSolution::last_iterate`].
//! - [`RootBatch::roots`] is the all-or-nothing path: it returns
//!   `Err(`[`RootBatchFailure`]`)` naming the failure count and the first
//!   failing lane, rather than a plausible-looking `Vec<f64>`.
//!
//! **A non-converged lane is never clamped to a bracket endpoint and called a
//! root.** A lane that ran out of iterations reports its best iterate with
//! [`RootStatus::MaxIterations`]; a lane whose bracket does not straddle a root
//! reports [`RootStatus::NotBracketed`] and a `NaN` iterate, because there is no
//! honest number to return.
//!
//! # Why this module lives under `math/` and not `polynomial/`
//!
//! The bulk of it is a general scalar root finder over a caller-supplied
//! residual — `h(T) - h_target`, a Peng-Robinson compressibility, a saturation
//! curve. None of that is polynomial. `math/` is already the home of the
//! crate's iterative inversions ([`crate::math::inv_inc_gamma`] is a
//! Newton-refined inverse), so a general solver belongs beside them; a caller
//! inverting an enthalpy would not think to look in a module called
//! `polynomial`. The batched closed-form polynomial kernels are thin wrappers
//! over [`crate::polynomial`]'s existing per-equation solvers and are kept here
//! with the rest of the batching so that the crate has **one** batched
//! root-finding vocabulary rather than two dialects in two modules.
//!
//! # Units
//!
//! Everything here is dimensionless `f64`, for the same reason
//! [`crate::ldu_matrix::parallel`] and [`crate::krylov::vecops`] are: a generic
//! root finder has no single physical dimension. The abscissa `x` of one lane
//! may be a temperature in kelvin and of another a compressibility factor, and
//! the residual's dimension is whatever the caller's function returns.
//!
//! `uom` typing is **not stripped** to get here — it is applied at the boundary,
//! by the caller, exactly as the hybrid-backend epic requires: convert into the
//! batch, convert back out. The doctest on [`solve_newton_batch`] shows that
//! boundary explicitly, inverting a `uom`-typed enthalpy relation for a
//! `ThermodynamicTemperature`.
//!
//! # Cargo features and portability
//!
//! The `rayon` paths sit behind the crate's `parallel` feature, which is **off
//! by default**; with it off this module still compiles and every entry point
//! still works. `rayon` is pure Rust with no system component, so everything
//! here compiles and runs on `aarch64-linux-android` / Termux exactly as on
//! desktop. Nothing in this module is target-gated.
//!
//! # Example
//!
//! ```rust
//! use outram_foam_basic_lib::compute::ComputeBackend;
//! use outram_foam_basic_lib::math::parallel::{
//!     solve_bracketed_batch, RootMethod, RootProblem, RootSettings,
//! };
//!
//! // 4 independent problems: x^2 = k for k = 1, 2, 3, 4, each bracketed on [0, 4].
//! let targets = [1.0, 2.0, 3.0, 4.0];
//! let problems: Vec<RootProblem> = (0..4).map(|_| RootProblem::new(0.0, 4.0)).collect();
//!
//! let batch = solve_bracketed_batch(
//!     &problems,
//!     RootMethod::Brent,
//!     RootSettings::default(),
//!     ComputeBackend::CpuMulti,
//!     |i, x| x * x - targets[i],
//! );
//!
//! let roots = batch.roots().expect("all four lanes converge");
//! assert!((roots[1] - 2.0_f64.sqrt()).abs() < 1e-12);
//!
//! // Asking for multi-CPU gives a bit-for-bit identical answer, whether or not
//! // the `parallel` feature is compiled in.
//! let serial = solve_bracketed_batch(
//!     &problems,
//!     RootMethod::Brent,
//!     RootSettings::default(),
//!     ComputeBackend::Serial,
//!     |i, x| x * x - targets[i],
//! );
//! assert_eq!(roots, serial.roots().unwrap());
//! ```

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::compute::ComputeBackend;
use crate::polynomial::{CubicEqn, LinearEqn, QuadraticEqn, Roots};

/// The WGSL kernels for this module's **closed-form** polynomial solvers.
///
/// The iterative root finders are deliberately not there — a Rust closure
/// cannot be shipped to a shader. See the submodule docs.
#[cfg(all(feature = "gpu", not(target_os = "android")))]
pub mod gpu;

#[cfg(test)]
mod tests;

// ── Tuning constants ─────────────────────────────────────────────────────────

/// Number of lanes in one task of the **closed-form polynomial** kernels
/// ([`linear_roots_batch`], [`quadratic_roots_batch`], [`cubic_roots_batch`]).
///
/// `rayon` splits adaptively, so this is a *lower bound* on task granularity
/// rather than a fixed task size: it stops the splitter subdividing below a
/// block too small to pay for its own scheduling. A closed-form cubic solve is
/// on the order of tens of nanoseconds, so splitting down to a single equation
/// would spend more time in the scheduler than in Cardano's formula.
///
/// The iterative solvers deliberately do **not** use a floor — see the
/// module-level "Load imbalance" section.
///
/// This constant affects wall clock only. It cannot change any value, because
/// every lane is computed independently of every other.
///
/// # Units
///
/// A count of polynomial equations, dimensionless.
pub const POLY_BLOCK: usize = 256;

/// Problem count below which a [`ComputeBackend::CpuMulti`] request runs the
/// **iterative** root-finding kernels ([`solve_bracketed_batch`],
/// [`solve_newton_batch`]) on the calling thread instead.
///
/// # Measured crossover
///
/// *Methodology.* Measured 2026-08-12 on this workspace's development machine,
/// `std::thread::available_parallelism()` = **4**, release build, `--features
/// parallel`, `rayon`'s global pool, otherwise-idle machine. Batches of `n`
/// independent problems `x^2 - k_i = 0` with `k_i` spread over `[1, 3)` so that
/// per-lane iteration counts differ, bracket `[0, 4]`, [`RootMethod::Brent`],
/// [`RootSettings::default`]. Best of 7 samples per point, reported as wall
/// clock for one whole batch. Produced by the `#[ignore]`d
/// `root_batch_crossover_benchmark` test in `parallel/tests.rs` and transcribed
/// from its printed output. The `cheap` closure is the two-flop residual above;
/// the `costly` closure adds a `ln`/`exp`/`sqrt` chain per evaluation, standing
/// in for a JANAF enthalpy. The serial columns are from the first run; the
/// `(repeat)` columns are the speed-ups from a second, independent run of the
/// same benchmark, carried alongside rather than averaged away because the
/// parallel column is far noisier than the serial one.
///
/// | Problems | cheap serial | cheap multi | speed-up | (repeat) | costly serial | costly multi | speed-up | (repeat) |
/// |---|---|---|---|---|---|---|---|---|
/// | 16 | 2.87 us | 34.77 us | 0.08x | 0.07x | 11.21 us | 42.19 us | 0.27x | 0.93x |
/// | 32 | 6.67 us | 36.60 us | 0.18x | 0.71x | 19.32 us | 43.55 us | 0.44x | 1.35x |
/// | 64 | 12.99 us | 45.16 us | 0.29x | 0.29x | 42.85 us | 62.99 us | 0.68x | 0.83x |
/// | 128 | 25.96 us | 57.24 us | 0.45x | 0.59x | 84.88 us | 86.65 us | 0.98x | 1.26x |
/// | 256 | 52.94 us | 42.79 us | 1.24x | 2.28x | 175.37 us | 126.70 us | 1.38x | 3.18x |
/// | 512 | 108.80 us | 68.63 us | 1.59x | 2.91x | 356.78 us | 170.71 us | 2.09x | 2.04x |
/// | 1 024 | 201.83 us | 127.52 us | 1.58x | 2.01x | 695.26 us | 284.70 us | 2.44x | 2.17x |
/// | 4 096 | 805.49 us | 501.42 us | 1.61x | 3.63x | 2774.06 us | 992.73 us | 2.79x | 3.58x |
/// | 16 384 | 3338.49 us | 1173.15 us | 2.85x | 2.71x | 11423.08 us | 3916.59 us | 2.92x | 2.83x |
/// | 65 536 | 13730.89 us | 5071.05 us | 2.71x | 2.94x | 45778.24 us | 16260.96 us | 2.82x | 3.02x |
///
/// *Result.* **256** is the smallest size at which *neither* closure lost in
/// *either* run — cheap 1.24x and 2.28x, costly 1.38x and 3.18x — and it is the
/// value this constant takes. At 128 the cheap closure lost both runs (0.45x,
/// 0.59x) while the costly one was a coin flip (0.98x, 1.26x), which is the
/// expected shape: a more expensive residual crosses over earlier because there
/// is more work per lane to amortise the same dispatch cost. The honest headline
/// is therefore that **the crossover depends on the caller's residual**, and
/// this single number is set for the cheapest residual a caller is likely to
/// pass, so that it is safe for both.
///
/// *Interpretation.* This is 16x **lower** than the crate-wide placeholder
/// [`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`] (4 096) and 512x lower than
/// [`crate::fields::parallel::FIELD_PARALLEL_CROSSOVER`] (131 072), and the
/// reason is structural rather than incidental. Field algebra does one or two
/// flops per element loaded and is memory-bandwidth bound, so extra cores buy
/// almost nothing; a root solve does tens of iterations of arithmetic per lane
/// against a handful of loaded bytes, so it is compute bound and threads scale
/// on it. Keeping the crate-wide placeholder here would forfeit a measured
/// 1.6x-2.4x on batches of 1 024.
///
/// # Relationship to [`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`]
///
/// That constant documents itself as an unmeasured placeholder and invites
/// per-kernel overrides. This is that override for the iterative root finders.
/// It is now the fourth measured value in the crate, and the spread across the
/// four (**256**, 4 096, 131 072, 262 144 — a factor of 1 024) is further
/// evidence that no single crate-wide threshold can be right. Offered as input
/// to bead `op-yvj.4.7`.
///
/// # Limitations
///
/// One machine, four logical cores, one closure family, idle. Not measured on
/// Android/Termux hardware and not on a many-core server. The crossover moves
/// with the caller's residual cost, so a caller that knows its own closure is
/// expensive is better off calling the kernel with an explicit
/// [`ComputeBackend`] than trusting a single number.
///
/// # Units
///
/// A count of independent root problems, dimensionless.
pub const ROOT_BATCH_MIN_PROBLEMS: usize = 256;

/// Equation count below which a [`ComputeBackend::CpuMulti`] request runs the
/// **closed-form polynomial** kernels on the calling thread instead.
///
/// # Measured crossover
///
/// *Methodology.* Same machine, build and conditions as
/// [`ROOT_BATCH_MIN_PROBLEMS`] (4 logical cores, release, `--features
/// parallel`, idle, best of 7 samples). Batches of `n` cubics
/// `(x - r1)(x - r2)(x - r3)` with pseudorandom real roots, solved by
/// [`cubic_roots_batch`]; timed as wall clock for one whole batch. Produced by
/// the `#[ignore]`d `poly_batch_crossover_benchmark` test in `parallel/tests.rs`
/// and transcribed from its printed output. The `(repeat)` column is the
/// speed-up from a second, independent run.
///
/// | Cubics | serial | CpuMulti | speed-up | (repeat) |
/// |---|---|---|---|---|
/// | 256 | 36.34 us | 36.43 us | 1.00x | 1.00x |
/// | 512 | 71.92 us | 110.25 us | 0.65x | 0.83x |
/// | 1 024 | 146.16 us | 96.48 us | 1.51x | 1.82x |
/// | 2 048 | 296.38 us | 189.08 us | 1.57x | 1.37x |
/// | 4 096 | 591.18 us | 170.46 us | 3.47x | 2.38x |
/// | 16 384 | 2377.28 us | 832.47 us | 2.86x | 1.90x |
/// | 65 536 | 10024.12 us | 3432.25 us | 2.92x | 2.76x |
/// | 262 144 | 40089.01 us | 11204.91 us | 3.58x | 2.87x |
///
/// *Result.* Set to **1 024**, the smallest measured size at which the parallel
/// path won in both runs (1.51x, 1.82x). At 512 it lost both (0.65x, 0.83x).
///
/// *Interpretation.* Note the 256-cubic row, where the two paths are *exactly*
/// break-even in both runs: [`POLY_BLOCK`] is 256, so there is a single task and
/// `rayon` never splits — the same artefact the `ldu_matrix` and `fields`
/// crossover tables show at their own block sizes, and a useful confirmation
/// that the granularity floor is doing what it claims.
///
/// This floor is 4x the iterative one, but **do not read that as a strong
/// structural difference**: the two kernels' per-lane costs are in fact
/// comparable. Dividing the largest serial timings by their lane counts gives
/// about **153 ns** per closed-form cubic against about **209 ns** per cheap
/// iterative lane. Both break even in the same shallow region, and the 4x gap
/// between the two constants reflects a conservative reading of a noisy
/// measurement — taking the smallest size that won in *both* runs — more than it
/// reflects physics. Both are still far below the crate-wide placeholder,
/// because both kernels are compute-dense next to a field add.
///
/// # Limitations
///
/// As [`ROOT_BATCH_MIN_PROBLEMS`]: one machine, four cores, idle, not measured
/// on Android. Measured with [`cubic_roots_batch`] only —
/// [`quadratic_roots_batch`] and [`linear_roots_batch`] are cheaper per lane and
/// so cross over *later*, and inherit this floor as an assumption rather than a
/// measurement.
///
/// # Units
///
/// A count of polynomial equations, dimensionless.
pub const POLY_ROOTS_MIN_EQUATIONS: usize = 1_024;

// ── Backend dispatch ─────────────────────────────────────────────────────────

/// Resolve a requested backend to the one this module will actually run, given
/// how much work there is.
///
/// Three reductions, in order: [`ComputeBackend::resolve`] degrades anything
/// whose feature is off; `Gpu` degrades again because this module has no GPU
/// kernel yet; and `CpuMulti` degrades to `Serial` below `min_work_items`. The
/// result is only ever `Serial` or `CpuMulti`, and none of the degradations can
/// change a returned value.
fn effective_backend(
    requested: ComputeBackend,
    work_items: usize,
    min_work_items: usize,
) -> ComputeBackend {
    let cpu = match requested.resolve() {
        ComputeBackend::Gpu => ComputeBackend::CpuMulti.resolve(),
        other => other,
    };
    match cpu {
        ComputeBackend::CpuMulti if work_items >= min_work_items => ComputeBackend::CpuMulti,
        _ => ComputeBackend::Serial,
    }
}

/// The [`ComputeBackend`] the iterative solvers would actually use for `n`
/// problems if asked for `requested` — without running anything.
///
/// Applies exactly the same reduction the kernels do (feature availability, no
/// GPU kernel here, and the [`ROOT_BATCH_MIN_PROBLEMS`] size floor), so what it
/// reports is what would run. Useful for logging and for benchmark harnesses.
///
/// # Arguments
///
/// - `requested` — the backend a caller would pass to [`solve_bracketed_batch`]
///   or [`solve_newton_batch`].
/// - `n` — the number of independent problems in the batch, dimensionless.
///
/// # Returns
///
/// Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`]; never
/// [`ComputeBackend::Gpu`], because no GPU kernel exists here yet.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::parallel::{root_batch_backend_for, ROOT_BATCH_MIN_PROBLEMS};
///
/// // Too small to thread, whatever was asked for.
/// assert_eq!(root_batch_backend_for(ComputeBackend::CpuMulti, 8), ComputeBackend::Serial);
///
/// // Big enough; the answer now depends only on whether `parallel` is compiled in.
/// let picked = root_batch_backend_for(ComputeBackend::CpuMulti, ROOT_BATCH_MIN_PROBLEMS);
/// assert!(picked.is_available());
/// ```
#[must_use]
pub fn root_batch_backend_for(requested: ComputeBackend, n: usize) -> ComputeBackend {
    effective_backend(requested, n, ROOT_BATCH_MIN_PROBLEMS)
}

/// The [`ComputeBackend`] the closed-form polynomial kernels would actually use
/// for `n` equations if asked for `requested` — without running anything.
///
/// The polynomial counterpart of [`root_batch_backend_for`], differing only in
/// using the [`POLY_ROOTS_MIN_EQUATIONS`] size floor.
///
/// # Arguments
///
/// - `requested` — the backend a caller would pass to [`cubic_roots_batch`] and
///   friends.
/// - `n` — the number of equations in the batch, dimensionless.
///
/// # Returns
///
/// Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`].
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::parallel::poly_roots_backend_for;
///
/// assert_eq!(poly_roots_backend_for(ComputeBackend::CpuMulti, 64), ComputeBackend::Serial);
/// assert_eq!(poly_roots_backend_for(ComputeBackend::Serial, 1 << 20), ComputeBackend::Serial);
/// ```
#[must_use]
pub fn poly_roots_backend_for(requested: ComputeBackend, n: usize) -> ComputeBackend {
    effective_backend(requested, n, POLY_ROOTS_MIN_EQUATIONS)
}

// ── Problem, settings, method ────────────────────────────────────────────────

/// Which derivative-free bracketed method [`solve_bracketed_batch`] should run.
///
/// Both methods keep a bracket `[a, b]` across which the residual changes sign
/// and shrink it until it meets the tolerance, so both are guaranteed to
/// converge on a continuous residual that is genuinely bracketed. They differ
/// only in how fast.
///
/// # Units
///
/// Dimensionless — a mode selector, not a quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RootMethod {
    /// Repeated bisection of the bracket.
    ///
    /// One residual evaluation per iteration, and the bracket width halves
    /// every time, so reaching a relative tolerance of `1e-12` from an
    /// order-unity bracket costs about 40 evaluations regardless of the
    /// function. Slow but utterly unshakeable: it cannot be defeated by a kink,
    /// a plateau, or a badly conditioned derivative.
    Bisection,
    /// Brent's method — inverse quadratic interpolation and the secant rule,
    /// falling back to bisection whenever the interpolated step misbehaves.
    ///
    /// The default, and the right choice unless you have a specific reason to
    /// want bisection's fixed cost. Superlinear on a smooth residual: the same
    /// `1e-12` tolerance typically costs 6-12 evaluations instead of 40, while
    /// retaining bisection's guarantee because every step that would leave the
    /// bracket or shrink it too slowly is replaced by a bisection step.
    ///
    /// Implemented from the published description of the algorithm (R. P.
    /// Brent, *Algorithms for Minimization without Derivatives*, Prentice-Hall
    /// 1973, chapter 4), not transcribed from any existing implementation.
    #[default]
    Brent,
}

/// One bracketed root problem: an interval believed to contain a root, plus an
/// optional starting iterate for the Newton solver.
///
/// # Fields and valid ranges
///
/// - `lo`, `hi` — the bracket. Both must be finite. They may be given in either
///   order; the solvers do not require `lo < hi`. The residual must have
///   opposite signs at the two ends (or be exactly zero at one of them),
///   otherwise the lane reports [`RootStatus::NotBracketed`] rather than
///   guessing.
/// - `guess` — the initial iterate for [`solve_newton_batch`]. Ignored by
///   [`solve_bracketed_batch`], which is driven entirely by the bracket. A
///   non-finite guess, or one outside `[lo, hi]`, is replaced by the bracket
///   midpoint rather than rejected — a bad guess is a performance problem, not
///   a correctness one, because the Newton solver is bracket-safeguarded.
///
/// # Units
///
/// Dimensionless `f64` in whatever units the caller's abscissa carries — kelvin
/// for a temperature inversion, dimensionless for a compressibility factor. See
/// the module-level "Units" section.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::math::parallel::RootProblem;
///
/// // Bracket only; the Newton guess defaults to the midpoint.
/// let p = RootProblem::new(300.0, 3000.0);
/// assert_eq!(p.guess, 1650.0);
///
/// // Or seed Newton from the previous timestep's answer.
/// let q = RootProblem::with_guess(300.0, 3000.0, 812.5);
/// assert_eq!(q.guess, 812.5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RootProblem {
    /// One end of the bracket. Finite; need not be less than `hi`.
    pub lo: f64,
    /// The other end of the bracket. Finite; need not be greater than `lo`.
    pub hi: f64,
    /// Starting iterate for [`solve_newton_batch`]; ignored by
    /// [`solve_bracketed_batch`].
    pub guess: f64,
}

impl RootProblem {
    /// A problem bracketed on `[lo, hi]`, with the Newton guess set to the
    /// bracket midpoint.
    ///
    /// # Arguments
    ///
    /// - `lo`, `hi` — the bracket ends, finite, in either order.
    #[must_use]
    pub fn new(lo: f64, hi: f64) -> Self {
        Self {
            lo,
            hi,
            guess: lo + 0.5 * (hi - lo),
        }
    }

    /// A problem bracketed on `[lo, hi]` with an explicit Newton starting
    /// iterate.
    ///
    /// Seeding from the previous timestep's converged value is the usual reason
    /// to reach for this: it typically halves the iteration count of a per-cell
    /// thermodynamic inversion.
    ///
    /// # Arguments
    ///
    /// - `lo`, `hi` — the bracket ends, finite, in either order.
    /// - `guess` — starting iterate. Silently replaced by the midpoint if it is
    ///   non-finite or outside the bracket.
    #[must_use]
    pub fn with_guess(lo: f64, hi: f64, guess: f64) -> Self {
        Self { lo, hi, guess }
    }
}

/// Convergence tolerances and the iteration cap, shared by both iterative
/// solvers.
///
/// A lane is converged when **either** the bracket (bisection, Brent) or the
/// step (Newton) has shrunk to `x_tol_abs + x_tol_rel * |x|`, **or** the
/// residual magnitude has fallen to `f_tol`. An exactly-zero residual always
/// converges regardless of tolerances.
///
/// # Fields and valid ranges
///
/// - `x_tol_abs` — absolute abscissa tolerance, in the abscissa's own units.
///   Must be `>= 0`. Guards the case `x ≈ 0`, where a purely relative tolerance
///   can never be met.
/// - `x_tol_rel` — relative abscissa tolerance, dimensionless, `>= 0`.
/// - `f_tol` — residual tolerance, in the residual's own units, `>= 0`. Default
///   `0.0`, i.e. **disabled**: with no knowledge of the residual's scale, a
///   nonzero default would be a guess. Set it when you know what "small enough"
///   means for your function; leaving it at zero simply means the abscissa
///   tolerance decides.
/// - `max_iterations` — hard cap. On reaching it a lane reports
///   [`RootStatus::MaxIterations`] and its best iterate. Must be `>= 1`.
///
/// A negative tolerance is not rejected; it simply can never be met, and the
/// lane will report [`RootStatus::MaxIterations`]. That is a truthful outcome
/// rather than a silent success.
///
/// # Defaults
///
/// `x_tol_abs = 1e-12`, `x_tol_rel = 1e-12`, `f_tol = 0.0`,
/// `max_iterations = 100`. One hundred iterations is comfortable for Brent
/// (typically 6-12) and adequate for bisection over any bracket narrower than
/// about `2^100` tolerances.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::math::parallel::RootSettings;
///
/// // Struct-update syntax keeps the defaults you did not mean to change.
/// let s = RootSettings {
///     max_iterations: 200,
///     ..RootSettings::default()
/// };
/// assert_eq!(s.x_tol_rel, 1e-12);
/// assert_eq!(s.max_iterations, 200);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RootSettings {
    /// Absolute abscissa tolerance, in the abscissa's units. `>= 0`.
    pub x_tol_abs: f64,
    /// Relative abscissa tolerance, dimensionless. `>= 0`.
    pub x_tol_rel: f64,
    /// Residual tolerance, in the residual's units. `0.0` disables it.
    pub f_tol: f64,
    /// Maximum iterations per lane before reporting
    /// [`RootStatus::MaxIterations`]. `>= 1`.
    pub max_iterations: u32,
}

impl Default for RootSettings {
    fn default() -> Self {
        Self {
            x_tol_abs: 1e-12,
            x_tol_rel: 1e-12,
            f_tol: 0.0,
            max_iterations: 100,
        }
    }
}

impl RootSettings {
    /// The abscissa tolerance at `x`: `x_tol_abs + x_tol_rel * |x|`.
    #[inline]
    fn x_tol(&self, x: f64) -> f64 {
        self.x_tol_abs + self.x_tol_rel * x.abs()
    }

    /// Whether a residual of this magnitude counts as converged.
    ///
    /// An exactly-zero residual always does, whatever `f_tol` is.
    #[inline]
    fn residual_small_enough(&self, f: f64) -> bool {
        f == 0.0 || f.abs() <= self.f_tol
    }
}

// ── Per-lane outcome ─────────────────────────────────────────────────────────

/// How one lane of a batch ended.
///
/// Only [`Converged`](Self::Converged) means the lane produced a root. Every
/// other variant is a failure that a caller must handle; see the module-level
/// "Non-convergence is reported, never swallowed" section for the accessors
/// that make it hard to skip.
///
/// | Variant | `last_iterate` | Meaning |
/// |---|---|---|
/// | [`Converged`](Self::Converged) | the root | a tolerance was met |
/// | [`MaxIterations`](Self::MaxIterations) | best iterate so far — **not** a root | ran out of iterations |
/// | [`NotBracketed`](Self::NotBracketed) | `NaN` | residual has the same sign at both bracket ends |
/// | [`InvalidBracket`](Self::InvalidBracket) | `NaN` | a bracket end is not finite |
/// | [`NotFinite`](Self::NotFinite) | the abscissa where it happened | the residual (or derivative) evaluated to `NaN`/infinity |
///
/// # Units
///
/// Dimensionless — a status tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RootStatus {
    /// A root was found to the requested tolerance.
    Converged,
    /// `max_iterations` was reached with no tolerance met. The reported iterate
    /// is the best one seen, and is **not** claimed to be a root.
    MaxIterations,
    /// The residual has the same (nonzero) sign at both ends of the bracket, so
    /// no root of a continuous residual is guaranteed to lie inside it. The
    /// solver refuses to guess and reports this instead of returning an
    /// endpoint.
    NotBracketed,
    /// A bracket end was infinite or `NaN`, so there is no interval to search.
    InvalidBracket,
    /// The residual — or, for Newton, the value half of the residual/derivative
    /// pair — evaluated to a non-finite number. Iteration stops immediately,
    /// because continuing would propagate the `NaN` into a plausible-looking
    /// answer.
    NotFinite,
}

impl RootStatus {
    /// Whether this status means the lane produced a usable root.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::math::parallel::RootStatus;
    ///
    /// assert!(RootStatus::Converged.is_converged());
    /// assert!(!RootStatus::MaxIterations.is_converged());
    /// ```
    #[must_use]
    pub fn is_converged(self) -> bool {
        matches!(self, Self::Converged)
    }

    /// A short human-readable label, for log lines and failure reports.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Converged => "converged",
            Self::MaxIterations => "max-iterations",
            Self::NotBracketed => "not-bracketed",
            Self::InvalidBracket => "invalid-bracket",
            Self::NotFinite => "not-finite",
        }
    }
}

/// The outcome of a single lane: its status, its iterate, its residual and the
/// work it took.
///
/// The fields are private on purpose. The only way to get a number that is
/// claimed to be a root is [`Self::root`], which returns `Option<f64>` and hands
/// back `Some` only for a converged lane. The raw iterate is available from
/// [`Self::last_iterate`], whose name is chosen so that using it as a root is a
/// visible decision in the calling code rather than an accident.
///
/// `Copy`, so it can be read out of a [`RootBatch`] without cloning.
///
/// # Units
///
/// [`Self::root`] and [`Self::last_iterate`] are in the abscissa's units;
/// [`Self::residual`] is in the residual's units; [`Self::iterations`] and
/// [`Self::bracket_width`] are a dimensionless count and an abscissa-unit width
/// respectively.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RootSolution {
    x: f64,
    f_x: f64,
    bracket_width: f64,
    iterations: u32,
    status: RootStatus,
}

impl RootSolution {
    /// The root, if this lane converged.
    ///
    /// `None` for every failure status. This is the accessor to reach for; the
    /// `Option` is the point.
    ///
    /// # Returns
    ///
    /// `Some(x)` in the abscissa's units when [`Self::status`] is
    /// [`RootStatus::Converged`], otherwise `None`.
    #[must_use]
    pub fn root(&self) -> Option<f64> {
        if self.status.is_converged() {
            Some(self.x)
        } else {
            None
        }
    }

    /// The last iterate the solver held, converged or not.
    ///
    /// **A root only when [`Self::status`] is [`RootStatus::Converged`].** For
    /// [`RootStatus::MaxIterations`] this is the best estimate available and is
    /// offered for diagnosis — for deciding whether the lane was close or wildly
    /// off — not as an answer. For [`RootStatus::NotBracketed`] and
    /// [`RootStatus::InvalidBracket`] it is `NaN`, because there is no honest
    /// number to report and returning a bracket endpoint would be a lie.
    #[must_use]
    pub fn last_iterate(&self) -> f64 {
        self.x
    }

    /// The residual at [`Self::last_iterate`], in the residual's units.
    ///
    /// `NaN` when no meaningful evaluation was reached.
    #[must_use]
    pub fn residual(&self) -> f64 {
        self.f_x
    }

    /// Width of the final bracket, in the abscissa's units.
    ///
    /// For Newton this is the width of the safeguarding bracket, not the final
    /// step. `NaN` when the lane never established a bracket.
    #[must_use]
    pub fn bracket_width(&self) -> f64 {
        self.bracket_width
    }

    /// Iterations performed on this lane, dimensionless.
    ///
    /// One iteration costs one residual evaluation, on top of the two bracket
    /// evaluations every lane pays up front (three for Newton, which also
    /// evaluates at its starting iterate). Useful for spotting that one lane in
    /// ten thousand which is dragging the whole batch.
    #[must_use]
    pub fn iterations(&self) -> u32 {
        self.iterations
    }

    /// How this lane ended.
    #[must_use]
    pub fn status(&self) -> RootStatus {
        self.status
    }

    /// Whether this lane produced a root.
    #[must_use]
    pub fn converged(&self) -> bool {
        self.status.is_converged()
    }

    /// A converged lane.
    #[inline]
    fn converged_at(x: f64, f_x: f64, bracket_width: f64, iterations: u32) -> Self {
        Self {
            x,
            f_x,
            bracket_width,
            iterations,
            status: RootStatus::Converged,
        }
    }

    /// A failed lane, carrying whatever diagnostic information exists.
    #[inline]
    fn failed(status: RootStatus, x: f64, f_x: f64, bracket_width: f64, iterations: u32) -> Self {
        Self {
            x,
            f_x,
            bracket_width,
            iterations,
            status,
        }
    }
}

/// A batch of `N` lane outcomes, in the same order as the problems handed in.
///
/// Lane `i` of the result corresponds to `problems[i]`, always — the parallel
/// path preserves order, so no index bookkeeping is needed.
///
/// # Getting numbers out
///
/// - [`Self::roots`] — all-or-nothing. `Ok(Vec<f64>)` only when every lane
///   converged; otherwise `Err(`[`RootBatchFailure`]`)`. Use this when a partial
///   answer is useless, which for a per-cell thermodynamic inversion it usually
///   is.
/// - [`Self::solutions`] — per-lane, when the caller wants to handle failures
///   individually (fall back to a wider bracket, flag the cell, sub-cycle).
///
/// # Units
///
/// See [`RootSolution`].
#[derive(Debug, Clone, PartialEq)]
pub struct RootBatch {
    solutions: Vec<RootSolution>,
}

impl RootBatch {
    /// Every lane's outcome, in problem order.
    #[must_use]
    pub fn solutions(&self) -> &[RootSolution] {
        &self.solutions
    }

    /// Consume the batch and take the outcomes.
    #[must_use]
    pub fn into_solutions(self) -> Vec<RootSolution> {
        self.solutions
    }

    /// Number of lanes, dimensionless.
    #[must_use]
    pub fn len(&self) -> usize {
        self.solutions.len()
    }

    /// Whether the batch has no lanes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.solutions.is_empty()
    }

    /// Lane `i`'s outcome, or `None` if `i` is out of range.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<RootSolution> {
        self.solutions.get(i).copied()
    }

    /// Whether every lane converged. Vacuously `true` for an empty batch.
    #[must_use]
    pub fn all_converged(&self) -> bool {
        self.solutions.iter().all(RootSolution::converged)
    }

    /// How many lanes failed to converge, dimensionless.
    #[must_use]
    pub fn failure_count(&self) -> usize {
        self.solutions.iter().filter(|s| !s.converged()).count()
    }

    /// The first failing lane and its outcome, if any.
    ///
    /// The natural thing to print when a batch fails: it names the cell to look
    /// at rather than just the count.
    #[must_use]
    pub fn first_failure(&self) -> Option<(usize, RootSolution)> {
        self.solutions
            .iter()
            .enumerate()
            .find(|(_, s)| !s.converged())
            .map(|(i, s)| (i, *s))
    }

    /// Every failing lane, as `(index, outcome)` pairs.
    ///
    /// Allocates, so prefer [`Self::first_failure`] or [`Self::failure_count`]
    /// on a hot path.
    #[must_use]
    pub fn failures(&self) -> Vec<(usize, RootSolution)> {
        self.solutions
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.converged())
            .map(|(i, s)| (i, *s))
            .collect()
    }

    /// All roots, or an error naming the failures — the all-or-nothing path.
    ///
    /// # Returns
    ///
    /// `Ok(v)` with `v[i]` the root of problem `i`, in the abscissa's units,
    /// when every lane converged. Otherwise `Err(`[`RootBatchFailure`]`)`
    /// carrying the failure count and the first failing lane. **No `Vec` of
    /// plausible-looking numbers is ever returned for a batch that contained a
    /// failure.**
    ///
    /// An empty batch returns `Ok(vec![])`.
    ///
    /// # Errors
    ///
    /// [`RootBatchFailure`] when one or more lanes did not converge.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::compute::ComputeBackend;
    /// use outram_foam_basic_lib::math::parallel::{
    ///     solve_bracketed_batch, RootMethod, RootProblem, RootSettings, RootStatus,
    /// };
    ///
    /// // Lane 1's bracket does not straddle a root of x^2 - 2.
    /// let problems = [RootProblem::new(0.0, 4.0), RootProblem::new(3.0, 4.0)];
    /// let batch = solve_bracketed_batch(
    ///     &problems,
    ///     RootMethod::Brent,
    ///     RootSettings::default(),
    ///     ComputeBackend::Serial,
    ///     |_, x| x * x - 2.0,
    /// );
    ///
    /// let err = batch.roots().expect_err("lane 1 is not bracketed");
    /// assert_eq!(err.failure_count, 1);
    /// assert_eq!(err.first_index, 1);
    /// assert_eq!(err.first_status, RootStatus::NotBracketed);
    ///
    /// // The converged lane is still individually readable.
    /// assert!(batch.solutions()[0].root().is_some());
    /// assert!(batch.solutions()[1].root().is_none());
    /// ```
    pub fn roots(&self) -> Result<Vec<f64>, RootBatchFailure> {
        if let Some((i, s)) = self.first_failure() {
            return Err(RootBatchFailure {
                total: self.solutions.len(),
                failure_count: self.failure_count(),
                first_index: i,
                first_status: s.status(),
                first_iterations: s.iterations(),
            });
        }
        Ok(self.solutions.iter().map(|s| s.x).collect())
    }
}

/// One or more lanes of a [`RootBatch`] did not converge.
///
/// Returned by [`RootBatch::roots`]. It names both the scale of the problem
/// (how many of how many) and a specific lane to look at, because "3 of 10 000
/// cells failed" is only actionable once you know *which* cell.
///
/// # Units
///
/// All counts and indices are dimensionless.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "{failure_count} of {total} root problems did not converge; \
     first failure at lane {first_index} with status {first_status:?} \
     after {first_iterations} iterations"
)]
pub struct RootBatchFailure {
    /// Number of lanes in the batch.
    pub total: usize,
    /// Number of lanes that did not converge.
    pub failure_count: usize,
    /// Index of the first non-converged lane.
    pub first_index: usize,
    /// Why that lane failed.
    pub first_status: RootStatus,
    /// Iterations that lane performed before giving up.
    pub first_iterations: u32,
}

// ── Iterative solvers — the public batch entry points ────────────────────────

/// Solve `N` independent bracketed root problems without a derivative, on the
/// chosen backend.
///
/// This is the entry point for the common case: a residual you can evaluate but
/// whose derivative you would rather not write, bracketed per lane. Both
/// [`RootMethod`]s are globally convergent on a continuous, genuinely bracketed
/// residual.
///
/// # Arguments
///
/// - `problems` — one [`RootProblem`] per lane. `problems[i]`'s bracket is used
///   with `f(i, ·)`. The `guess` field is ignored.
/// - `method` — [`RootMethod::Brent`] unless you specifically want bisection's
///   fixed, function-independent cost.
/// - `settings` — tolerances and iteration cap; see [`RootSettings`].
/// - `backend` — requested execution backend. What actually runs is
///   [`root_batch_backend_for`] applied to it: an unavailable backend degrades,
///   `Gpu` degrades (no GPU kernel here yet), and a batch below
///   [`ROOT_BATCH_MIN_PROBLEMS`] runs serially. None of those changes the
///   answer.
/// - `f` — the residual. `f(i, x)` must return the value of lane `i`'s function
///   at abscissa `x`. It **must be a pure deterministic function of its
///   arguments** — see the module-level "Determinism" section for what breaks if
///   it is not. It is called from multiple threads on the `CpuMulti` path,
///   hence the `Sync` bound; the bound is present in both feature builds so
///   that enabling `parallel` never changes a public signature.
///
/// # Returns
///
/// A [`RootBatch`] with one [`RootSolution`] per problem, in problem order. An
/// empty `problems` slice returns an empty batch and calls `f` zero times.
///
/// # Determinism
///
/// Bit-for-bit identical on [`ComputeBackend::Serial`] and
/// [`ComputeBackend::CpuMulti`], at any thread count, for a pure `f`. Both
/// backends run the same per-lane kernel and no arithmetic crosses lanes.
///
/// # Non-convergence
///
/// Per lane, never swallowed. A lane whose bracket does not straddle a root
/// reports [`RootStatus::NotBracketed`] and a `NaN` iterate; a lane that
/// exhausts `max_iterations` reports [`RootStatus::MaxIterations`] and its best
/// iterate, which [`RootSolution::root`] still refuses to hand out as a root.
/// [`RootBatch::roots`] turns any failure into an `Err`.
///
/// # Verification
///
/// *Methodology.* The batch is checked against closed-form analytic roots, which
/// exist for the test residuals and so are an exact oracle rather than another
/// implementation. `(x - 1)(x - 2)(x - 3)` is solved on brackets isolating each
/// of its three known roots, and `x^2 - k` against `sqrt(k)`; the pass criterion
/// is `|x_computed - x_analytic| <= 1e-12` and `status == Converged` on every
/// lane.
///
/// *Results, measured 2026-08-12 by `bracketed_matches_analytic_cubic_roots` and
/// `brent_matches_analytic_sqrt` in `parallel/tests.rs`, release build:* worst
/// absolute error over the three cubic roots **0.000000e0** for *both*
/// [`RootMethod::Brent`] and [`RootMethod::Bisection`] — every lane recovered
/// 1.0, 2.0 and 3.0 exactly — and worst absolute error over the 64 `sqrt(k)`
/// lanes **1.776357e-15**. All lanes converged. *Interpretation:* the integer
/// cubic roots are exactly representable and both methods land on them bit for
/// bit; the `sqrt(k)` roots are irrational, and 1.78e-15 is 8 units in the last
/// place at `k = 64`, i.e. the rounding floor. The iteration contributes no
/// error of its own beyond the residual's conditioning.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::parallel::{
///     solve_bracketed_batch, RootMethod, RootProblem, RootSettings,
/// };
///
/// // Three lanes, each isolating one root of (x-1)(x-2)(x-3).
/// let cubic = |x: f64| (x - 1.0) * (x - 2.0) * (x - 3.0);
/// let problems = [
///     RootProblem::new(0.5, 1.5),
///     RootProblem::new(1.5, 2.5),
///     RootProblem::new(2.5, 3.5),
/// ];
///
/// let batch = solve_bracketed_batch(
///     &problems,
///     RootMethod::Brent,
///     RootSettings::default(),
///     ComputeBackend::CpuMulti,
///     |_, x| cubic(x),
/// );
///
/// let roots = batch.roots().expect("all lanes converge");
/// for (got, want) in roots.iter().zip([1.0, 2.0, 3.0]) {
///     assert!((got - want).abs() < 1e-12, "got {got}, want {want}");
/// }
/// ```
#[must_use]
pub fn solve_bracketed_batch<F>(
    problems: &[RootProblem],
    method: RootMethod,
    settings: RootSettings,
    backend: ComputeBackend,
    f: F,
) -> RootBatch
where
    F: Fn(usize, f64) -> f64 + Sync,
{
    solve_bracketed_batch_min(
        problems,
        method,
        settings,
        backend,
        ROOT_BATCH_MIN_PROBLEMS,
        f,
    )
}

/// [`solve_bracketed_batch`] with the size floor supplied by the caller.
///
/// Exists so the crossover benchmark can measure the multi-CPU path *below*
/// [`ROOT_BATCH_MIN_PROBLEMS`] — the only way to find where the crossover
/// actually is — and so the cross-backend bitwise tests are not vacuous on small
/// batches. Not public: production callers get the measured floor.
pub(crate) fn solve_bracketed_batch_min<F>(
    problems: &[RootProblem],
    method: RootMethod,
    settings: RootSettings,
    backend: ComputeBackend,
    min_problems: usize,
    f: F,
) -> RootBatch
where
    F: Fn(usize, f64) -> f64 + Sync,
{
    let n = problems.len();
    let solutions: Vec<RootSolution> = match effective_backend(backend, n, min_problems) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => problems
            .par_iter()
            .enumerate()
            // No `min_len` floor: per-lane iteration counts vary, so the
            // splitter is left free to steal down to a single lane.
            .map(|(i, p)| solve_one_bracketed(i, *p, method, settings, &f))
            .collect(),
        _ => problems
            .iter()
            .enumerate()
            .map(|(i, p)| solve_one_bracketed(i, *p, method, settings, &f))
            .collect(),
    };
    RootBatch { solutions }
}

/// Solve `N` independent root problems by bracket-safeguarded Newton iteration,
/// on the chosen backend.
///
/// Use this when the derivative is cheap to obtain alongside the value — which
/// for a thermodynamic inversion it is, because `dh/dT` is the specific heat the
/// same evaluation already computed. It is typically 2-4x fewer function
/// evaluations than [`RootMethod::Brent`] on a smooth residual.
///
/// # Why it is safeguarded, and not bare Newton
///
/// Bare Newton has no guarantee at all: a small derivative throws the iterate to
/// infinity, and a nearby inflection makes it cycle. Neither failure is
/// acceptable in a per-cell inversion where one bad cell out of a million stalls
/// the timestep. This solver therefore keeps the bracket that Newton lacks: it
/// takes the Newton step **only** when the step lands inside the current bracket
/// and is shrinking the interval fast enough, and bisects otherwise. The result
/// converges on anything a bisection would converge on, at Newton's rate
/// wherever Newton behaves.
///
/// # Arguments
///
/// - `problems` — one [`RootProblem`] per lane. Both the bracket **and** the
///   `guess` are used: the bracket safeguards, the guess starts the iteration.
///   A guess outside the bracket or non-finite is replaced by the midpoint.
/// - `settings` — tolerances and iteration cap. The abscissa tolerance is tested
///   against the **step**, `|dx| <= x_tol_abs + x_tol_rel * |x|`.
/// - `backend` — requested execution backend; see [`root_batch_backend_for`].
/// - `fdf` — `fdf(i, x)` returns `(value, derivative)` of lane `i`'s residual at
///   `x`. Returning both together is deliberate: an enthalpy and its specific
///   heat come out of one polynomial evaluation, and asking for them separately
///   would double the cost. Must be a pure deterministic function; see the
///   module-level "Determinism" section.
///
/// # Returns
///
/// A [`RootBatch`] with one [`RootSolution`] per problem, in problem order.
///
/// # Determinism
///
/// Bit-for-bit identical across backends and thread counts for a pure `fdf`, on
/// the same terms as [`solve_bracketed_batch`].
///
/// # Non-convergence
///
/// As [`solve_bracketed_batch`], plus one Newton-specific case: a zero or
/// non-finite derivative is **not** a failure — the lane simply bisects that
/// iteration and carries on. Only a non-finite *value* stops the lane, with
/// [`RootStatus::NotFinite`].
///
/// # Verification
///
/// *Methodology.* Checked against the analytic root of `x^2 - k = 0`, i.e.
/// `sqrt(k)`, over 64 lanes with `k` spread across `[1, 65]`, and against the
/// three known roots of `(x - 1)(x - 2)(x - 3)`; pass criterion
/// `|x_computed - x_analytic| <= 1e-12` with `status == Converged` on every
/// lane. Separately checked against [`RootMethod::Brent`] on the same problems,
/// with the iteration counts of both recorded.
///
/// *Results, measured 2026-08-12 by `newton_matches_analytic_roots` and
/// `newton_beats_brent_on_iterations` in `parallel/tests.rs`, release build:*
/// worst absolute error over the 64 `sqrt(k)` lanes **8.881784e-16**; worst over
/// the three cubic roots **0.000000e0** (all three recovered exactly); mean
/// iterations **7.73** for safeguarded Newton against **13.14** for Brent on the
/// same 64 lanes. *Interpretation:* Newton reaches the same rounding-floor
/// accuracy — in fact 2x better than Brent on the `sqrt(k)` family — in 59% of
/// Brent's iterations, which is the expected quadratic-versus-superlinear margin
/// and is the reason to pay for a derivative. Note the counts include the
/// safeguard's bisection steps, which is why they are higher than a textbook
/// bare-Newton count on the same problem.
///
/// # Example — the `uom` boundary
///
/// The batch is dimensionless, and the caller converts at its edge. This lane
/// inverts a linear `h(T)` relation for a `ThermodynamicTemperature`, keeping
/// `uom` typing on both sides of the call:
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::parallel::{
///     solve_newton_batch, RootProblem, RootSettings,
/// };
/// use uom::si::f64::{AvailableEnergy, ThermodynamicTemperature};
/// use uom::si::available_energy::joule_per_kilogram;
/// use uom::si::thermodynamic_temperature::kelvin;
///
/// // h(T) = cp * T with cp = 1000 J/(kg K); invert for two target enthalpies.
/// let cp = 1000.0_f64; // J / (kg K)
/// let targets = [
///     AvailableEnergy::new::<joule_per_kilogram>(400_000.0),
///     AvailableEnergy::new::<joule_per_kilogram>(900_000.0),
/// ];
///
/// // Convert in: brackets in kelvin, as plain f64.
/// let lo = ThermodynamicTemperature::new::<kelvin>(200.0);
/// let hi = ThermodynamicTemperature::new::<kelvin>(3000.0);
/// let problems = [
///     RootProblem::new(lo.get::<kelvin>(), hi.get::<kelvin>()),
///     RootProblem::new(lo.get::<kelvin>(), hi.get::<kelvin>()),
/// ];
///
/// let batch = solve_newton_batch(
///     &problems,
///     RootSettings::default(),
///     ComputeBackend::CpuMulti,
///     |i, t| (cp * t - targets[i].get::<joule_per_kilogram>(), cp),
/// );
///
/// // Convert out: back to a typed temperature.
/// let temperatures: Vec<ThermodynamicTemperature> = batch
///     .roots()
///     .expect("both lanes converge")
///     .into_iter()
///     .map(ThermodynamicTemperature::new::<kelvin>)
///     .collect();
///
/// assert!((temperatures[0].get::<kelvin>() - 400.0).abs() < 1e-9);
/// assert!((temperatures[1].get::<kelvin>() - 900.0).abs() < 1e-9);
/// ```
#[must_use]
pub fn solve_newton_batch<F>(
    problems: &[RootProblem],
    settings: RootSettings,
    backend: ComputeBackend,
    fdf: F,
) -> RootBatch
where
    F: Fn(usize, f64) -> (f64, f64) + Sync,
{
    solve_newton_batch_min(problems, settings, backend, ROOT_BATCH_MIN_PROBLEMS, fdf)
}

/// [`solve_newton_batch`] with the size floor supplied by the caller; see
/// [`solve_bracketed_batch_min`] for why the `_min` variants exist.
pub(crate) fn solve_newton_batch_min<F>(
    problems: &[RootProblem],
    settings: RootSettings,
    backend: ComputeBackend,
    min_problems: usize,
    fdf: F,
) -> RootBatch
where
    F: Fn(usize, f64) -> (f64, f64) + Sync,
{
    let n = problems.len();
    let solutions: Vec<RootSolution> = match effective_backend(backend, n, min_problems) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => problems
            .par_iter()
            .enumerate()
            .map(|(i, p)| solve_one_newton(i, *p, settings, &fdf))
            .collect(),
        _ => problems
            .iter()
            .enumerate()
            .map(|(i, p)| solve_one_newton(i, *p, settings, &fdf))
            .collect(),
    };
    RootBatch { solutions }
}

// ── Per-lane kernels — one implementation, both backends ─────────────────────

/// Evaluate both bracket ends and classify the lane before any iteration.
///
/// Returns `Err(solution)` when the lane is already decided — an invalid or
/// non-finite bracket, a root sitting exactly on an endpoint, or a bracket that
/// does not straddle a sign change. Returns `Ok((a, fa, b, fb))` otherwise, with
/// `fa` and `fb` finite, nonzero and of opposite sign.
#[inline]
fn prepare_bracket<F>(i: usize, p: RootProblem, f: &F) -> Result<(f64, f64, f64, f64), RootSolution>
where
    F: Fn(usize, f64) -> f64,
{
    let nan = f64::NAN;
    if !p.lo.is_finite() || !p.hi.is_finite() {
        return Err(RootSolution::failed(
            RootStatus::InvalidBracket,
            nan,
            nan,
            nan,
            0,
        ));
    }

    let (a, b) = (p.lo, p.hi);
    let fa = f(i, a);
    let fb = f(i, b);
    let width = (b - a).abs();

    if !fa.is_finite() {
        return Err(RootSolution::failed(RootStatus::NotFinite, a, fa, width, 0));
    }
    if !fb.is_finite() {
        return Err(RootSolution::failed(RootStatus::NotFinite, b, fb, width, 0));
    }
    // A root exactly on an endpoint is a root, not a degenerate bracket.
    if fa == 0.0 {
        return Err(RootSolution::converged_at(a, fa, width, 0));
    }
    if fb == 0.0 {
        return Err(RootSolution::converged_at(b, fb, width, 0));
    }
    // Same sign at both ends: no sign change is guaranteed inside, so refuse.
    if (fa > 0.0) == (fb > 0.0) {
        return Err(RootSolution::failed(
            RootStatus::NotBracketed,
            nan,
            nan,
            width,
            0,
        ));
    }
    Ok((a, fa, b, fb))
}

/// The single-lane derivative-free solve that **both** backends run.
///
/// `#[inline]` so the serial loop and the `rayon` map compile to the same inner
/// code — part of why the two backends agree bit for bit.
#[inline]
fn solve_one_bracketed<F>(
    i: usize,
    p: RootProblem,
    method: RootMethod,
    settings: RootSettings,
    f: &F,
) -> RootSolution
where
    F: Fn(usize, f64) -> f64,
{
    let (a, fa, b, fb) = match prepare_bracket(i, p, f) {
        Ok(v) => v,
        Err(decided) => return decided,
    };
    match method {
        RootMethod::Bisection => bisect_lane(i, a, fa, b, settings, f),
        RootMethod::Brent => brent_lane(i, a, fa, b, fb, settings, f),
    }
}

/// Repeated bisection of a bracket known to straddle a sign change.
#[inline]
fn bisect_lane<F>(
    i: usize,
    mut a: f64,
    mut fa: f64,
    mut b: f64,
    settings: RootSettings,
    f: &F,
) -> RootSolution
where
    F: Fn(usize, f64) -> f64,
{
    let mut m = a + 0.5 * (b - a);
    let mut fm = fa;
    for iteration in 1..=settings.max_iterations {
        m = a + 0.5 * (b - a);
        fm = f(i, m);
        if !fm.is_finite() {
            return RootSolution::failed(RootStatus::NotFinite, m, fm, (b - a).abs(), iteration);
        }
        let width = (b - a).abs();
        if settings.residual_small_enough(fm) || width <= settings.x_tol(m) {
            return RootSolution::converged_at(m, fm, width, iteration);
        }
        if (fm > 0.0) == (fa > 0.0) {
            a = m;
            fa = fm;
        } else {
            b = m;
        }
    }
    RootSolution::failed(
        RootStatus::MaxIterations,
        m,
        fm,
        (b - a).abs(),
        settings.max_iterations,
    )
}

/// Brent's method on a bracket known to straddle a sign change.
///
/// Implemented from the published description of the algorithm (Brent 1973,
/// chapter 4): inverse quadratic interpolation through the three most recent
/// iterates, the secant rule when two of them coincide in value, and a
/// bisection step whenever the interpolated point falls outside the middle of
/// the bracket or the interval is not shrinking fast enough. The bracket
/// invariant — `b` is the best estimate, `a` the counter-bracket, and the
/// residual changes sign between them — is maintained at every step, so the
/// method inherits bisection's guarantee.
#[inline]
fn brent_lane<F>(
    i: usize,
    a_in: f64,
    fa_in: f64,
    b_in: f64,
    fb_in: f64,
    settings: RootSettings,
    f: &F,
) -> RootSolution
where
    F: Fn(usize, f64) -> f64,
{
    // `b` is kept as the better of the two estimates.
    let (mut a, mut fa, mut b, mut fb) = if fa_in.abs() < fb_in.abs() {
        (b_in, fb_in, a_in, fa_in)
    } else {
        (a_in, fa_in, b_in, fb_in)
    };

    // `c` is the previous iterate, `d` the one before that.
    let mut c = a;
    let mut fc = fa;
    let mut d = a;
    let mut used_bisection = true;

    for iteration in 1..=settings.max_iterations {
        let width = (b - a).abs();
        let tol = settings.x_tol(b);
        if settings.residual_small_enough(fb) || width <= tol {
            return RootSolution::converged_at(b, fb, width, iteration - 1);
        }

        // Interpolate: inverse quadratic through (a, fa), (b, fb), (c, fc) when
        // the three residuals are distinct; otherwise the secant through the
        // two most recent.
        let mut s = if fa != fc && fb != fc {
            a * fb * fc / ((fa - fb) * (fa - fc))
                + b * fa * fc / ((fb - fa) * (fb - fc))
                + c * fa * fb / ((fc - fa) * (fc - fb))
        } else {
            b - fb * (b - a) / (fb - fa)
        };

        // Reject the interpolated step unless it lands in the middle half-ish of
        // the bracket and is shrinking the interval at least geometrically.
        let quarter = (3.0 * a + b) / 4.0;
        let (lo, hi) = if quarter < b {
            (quarter, b)
        } else {
            (b, quarter)
        };
        let outside = !(s > lo && s < hi);
        let stalling = if used_bisection {
            (s - b).abs() >= (b - c).abs() * 0.5 || (b - c).abs() < tol
        } else {
            (s - b).abs() >= (c - d).abs() * 0.5 || (c - d).abs() < tol
        };
        if outside || stalling || !s.is_finite() {
            s = a + 0.5 * (b - a);
            used_bisection = true;
        } else {
            used_bisection = false;
        }

        let fs = f(i, s);
        if !fs.is_finite() {
            return RootSolution::failed(RootStatus::NotFinite, s, fs, width, iteration);
        }

        d = c;
        c = b;
        fc = fb;

        // Keep the sign change: replace whichever end shares `s`'s sign.
        if (fa > 0.0) != (fs > 0.0) {
            b = s;
            fb = fs;
        } else {
            a = s;
            fa = fs;
        }
        // Re-establish "b is the better estimate".
        if fa.abs() < fb.abs() {
            std::mem::swap(&mut a, &mut b);
            std::mem::swap(&mut fa, &mut fb);
        }
        // An exact hit ends it immediately, whatever the tolerances say.
        if fb == 0.0 {
            return RootSolution::converged_at(b, fb, (b - a).abs(), iteration);
        }
    }

    RootSolution::failed(
        RootStatus::MaxIterations,
        b,
        fb,
        (b - a).abs(),
        settings.max_iterations,
    )
}

/// The single-lane bracket-safeguarded Newton solve that both backends run.
///
/// Keeps `[x_lo, x_hi]` oriented so the residual is negative at `x_lo` and
/// positive at `x_hi`. Each iteration takes the Newton step when it lands
/// inside that interval and is at least halving the step size, and bisects
/// otherwise — so a zero, non-finite or misleading derivative costs one wasted
/// bisection rather than a diverged lane.
#[inline]
fn solve_one_newton<F>(i: usize, p: RootProblem, settings: RootSettings, fdf: &F) -> RootSolution
where
    F: Fn(usize, f64) -> (f64, f64) + Sync,
{
    // Reuse the derivative-free bracket preparation by projecting to the value.
    let value_only = |j: usize, x: f64| fdf(j, x).0;
    let (a, fa, b, _fb) = match prepare_bracket(i, p, &value_only) {
        Ok(v) => v,
        Err(decided) => return decided,
    };

    // Orient so that f(x_lo) < 0 < f(x_hi).
    let (mut x_lo, mut x_hi) = if fa < 0.0 { (a, b) } else { (b, a) };

    let mid = a + 0.5 * (b - a);
    let in_bracket = |x: f64| {
        let (min, max) = if a < b { (a, b) } else { (b, a) };
        x.is_finite() && x >= min && x <= max
    };
    let mut x = if in_bracket(p.guess) { p.guess } else { mid };

    let (mut fx, mut dfx) = fdf(i, x);
    if !fx.is_finite() {
        return RootSolution::failed(RootStatus::NotFinite, x, fx, (b - a).abs(), 0);
    }
    if settings.residual_small_enough(fx) {
        return RootSolution::converged_at(x, fx, (x_hi - x_lo).abs(), 0);
    }

    let mut dx_previous = (b - a).abs();
    let mut dx = dx_previous;

    for iteration in 1..=settings.max_iterations {
        // Reject the Newton step if it would leave the bracket, if it is not at
        // least halving the step, or if the derivative is unusable.
        let leaves_bracket = ((x - x_hi) * dfx - fx) * ((x - x_lo) * dfx - fx) > 0.0;
        let too_slow = (2.0 * fx).abs() > (dx_previous * dfx).abs();
        if dfx == 0.0 || !dfx.is_finite() || leaves_bracket || too_slow {
            dx_previous = dx;
            dx = 0.5 * (x_hi - x_lo);
            x = x_lo + dx;
        } else {
            dx_previous = dx;
            dx = fx / dfx;
            x -= dx;
        }

        let (nfx, ndfx) = fdf(i, x);
        if !nfx.is_finite() {
            return RootSolution::failed(
                RootStatus::NotFinite,
                x,
                nfx,
                (x_hi - x_lo).abs(),
                iteration,
            );
        }
        fx = nfx;
        dfx = ndfx;

        if settings.residual_small_enough(fx) || dx.abs() <= settings.x_tol(x) {
            return RootSolution::converged_at(x, fx, (x_hi - x_lo).abs(), iteration);
        }

        if fx < 0.0 {
            x_lo = x;
        } else {
            x_hi = x;
        }
    }

    RootSolution::failed(
        RootStatus::MaxIterations,
        x,
        fx,
        (x_hi - x_lo).abs(),
        settings.max_iterations,
    )
}

// ── Closed-form polynomial roots, batched ────────────────────────────────────

/// Roots of `N` independent linear equations `a x + b = 0`, on the chosen
/// backend.
///
/// A batched wrapper over [`LinearEqn::roots`] — the per-equation closed-form
/// solver is unchanged and is not reimplemented here. Each lane is an
/// independent handful of floating-point operations, so this is the
/// branch-lightest kernel in the module.
///
/// # Arguments
///
/// - `eqns` — one [`LinearEqn`] per lane. Coefficients are dimensionless `f64`,
///   as everywhere in [`crate::polynomial`].
/// - `backend` — requested execution backend; see [`poly_roots_backend_for`] for
///   what will actually run.
///
/// # Returns
///
/// One [`Roots<1>`] per equation, in input order. Degenerate cases are tagged
/// rather than hidden: `|a| < VSMALL` yields a `Nan`-tagged root and an
/// overflowing quotient yields `PosInf`/`NegInf`, exactly as the scalar solver
/// does.
///
/// # Determinism
///
/// Bit-for-bit identical to calling [`LinearEqn::roots`] in a loop, on every
/// backend and at any thread count: each lane is one independent closed-form
/// evaluation.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::parallel::linear_roots_batch;
/// use outram_foam_basic_lib::polynomial::{LinearEqn, RootType};
///
/// // 2x - 4 = 0  and  3x + 9 = 0
/// let eqns = [LinearEqn::new(2.0, -4.0), LinearEqn::new(3.0, 9.0)];
/// let roots = linear_roots_batch(&eqns, ComputeBackend::CpuMulti);
///
/// assert_eq!(roots[0].root_type(0), RootType::Real);
/// assert_eq!(roots[0].get(0), 2.0);
/// assert_eq!(roots[1].get(0), -3.0);
/// ```
#[must_use]
pub fn linear_roots_batch(eqns: &[LinearEqn], backend: ComputeBackend) -> Vec<Roots<1>> {
    linear_roots_batch_min(eqns, backend, POLY_ROOTS_MIN_EQUATIONS)
}

/// [`linear_roots_batch`] with a caller-supplied size floor; see
/// [`solve_bracketed_batch_min`] for why the `_min` variants exist.
pub(crate) fn linear_roots_batch_min(
    eqns: &[LinearEqn],
    backend: ComputeBackend,
    min_equations: usize,
) -> Vec<Roots<1>> {
    match effective_backend(backend, eqns.len(), min_equations) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => eqns
            .par_iter()
            .with_min_len(POLY_BLOCK)
            .map(LinearEqn::roots)
            .collect(),
        _ => eqns.iter().map(LinearEqn::roots).collect(),
    }
}

/// Roots of `N` independent quadratic equations `a x^2 + b x + c = 0`, on the
/// chosen backend.
///
/// A batched wrapper over [`QuadraticEqn::roots`], which uses a
/// Kahan-compensated discriminant; the per-equation solver is unchanged and is
/// not reimplemented here.
///
/// # Arguments
///
/// - `eqns` — one [`QuadraticEqn`] per lane, dimensionless coefficients.
/// - `backend` — requested execution backend.
///
/// # Returns
///
/// One [`Roots<2>`] per equation, in input order. Two `Real` roots when the
/// discriminant is positive; a `Complex`-tagged (real part, imaginary part) pair
/// when it is negative; one `Real` plus one `Nan` when `|a|` underflows and the
/// equation is really linear.
///
/// # Determinism
///
/// Bit-for-bit identical to calling [`QuadraticEqn::roots`] in a loop, on every
/// backend and at any thread count.
///
/// # Verification
///
/// *Methodology.* Every lane's real roots are substituted back into their own
/// equation and the residual `|a x^2 + b x + c|` compared against zero, over
/// 4 096 equations built as `(x - r1)(x - r2)` with pseudorandom real roots on
/// `[-8, 8)` from a fixed-seed xorshift64\*; pass criterion: residual
/// `<= 1e-9` and bitwise equality against the per-equation scalar solver.
///
/// *Result, measured 2026-08-12 by `quadratic_batch_residuals_are_analytic` in
/// `parallel/tests.rs`, release build:* worst residual **7.105427e-15** over
/// 8 192 real roots; bitwise equality against the scalar solver held on every
/// lane. *Interpretation:* the batching adds nothing to the scalar solver's own
/// error, which is what "bit-for-bit identical" means measured rather than
/// argued.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::parallel::quadratic_roots_batch;
/// use outram_foam_basic_lib::polynomial::{QuadraticEqn, RootType};
///
/// // (x - 1)(x - 2) = x^2 - 3x + 2
/// let eqns = [QuadraticEqn::new(1.0, -3.0, 2.0)];
/// let roots = quadratic_roots_batch(&eqns, ComputeBackend::CpuMulti);
///
/// let mut real: Vec<f64> = (0..2)
///     .filter(|&k| roots[0].root_type(k) == RootType::Real)
///     .map(|k| roots[0].get(k))
///     .collect();
/// real.sort_by(f64::total_cmp);
/// assert!((real[0] - 1.0).abs() < 1e-12);
/// assert!((real[1] - 2.0).abs() < 1e-12);
/// ```
#[must_use]
pub fn quadratic_roots_batch(eqns: &[QuadraticEqn], backend: ComputeBackend) -> Vec<Roots<2>> {
    quadratic_roots_batch_min(eqns, backend, POLY_ROOTS_MIN_EQUATIONS)
}

/// [`quadratic_roots_batch`] with a caller-supplied size floor; see
/// [`solve_bracketed_batch_min`] for why the `_min` variants exist.
pub(crate) fn quadratic_roots_batch_min(
    eqns: &[QuadraticEqn],
    backend: ComputeBackend,
    min_equations: usize,
) -> Vec<Roots<2>> {
    match effective_backend(backend, eqns.len(), min_equations) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => eqns
            .par_iter()
            .with_min_len(POLY_BLOCK)
            .map(QuadraticEqn::roots)
            .collect(),
        _ => eqns.iter().map(QuadraticEqn::roots).collect(),
    }
}

/// Roots of `N` independent cubic equations `a x^3 + b x^2 + c x + d = 0`, on
/// the chosen backend.
///
/// A batched wrapper over [`CubicEqn::roots`] — the depressed-cubic Cardano
/// solver with Kahan-compensated discriminants, unchanged and not reimplemented
/// here.
///
/// This is the kernel a cubic-equation-of-state caller wants: a Peng-Robinson or
/// Redlich-Kwong compressibility factor is the root of a cubic in `Z`, one cubic
/// per cell, and the whole field can be solved in a single call.
///
/// # Arguments
///
/// - `eqns` — one [`CubicEqn`] per lane, dimensionless coefficients.
/// - `backend` — requested execution backend; see [`poly_roots_backend_for`].
///
/// # Returns
///
/// One [`Roots<3>`] per equation, in input order. Three `Real` roots, or one
/// `Real` plus a `Complex`-tagged (real, imaginary) pair, or a triple root, or —
/// when `|a|` underflows — the quadratic's roots plus a `Nan` slot. The tags are
/// the caller's guide; do not read a slot without checking
/// [`Roots::root_type`].
///
/// There is no `_into` variant, unlike [`crate::ldu_matrix::parallel`]'s
/// kernels: [`Roots`] has no `Default`, so a caller-owned output buffer would
/// have to be pre-filled with a dummy value, and the per-lane Cardano solve
/// dominates the allocation anyway.
///
/// # Determinism
///
/// Bit-for-bit identical to calling [`CubicEqn::roots`] in a loop, on every
/// backend and at any thread count. Each lane is one independent closed-form
/// evaluation; no arithmetic crosses lanes.
///
/// # Verification
///
/// *Methodology.* Cubics are constructed from **known** roots as
/// `(x - r1)(x - r2)(x - r3)` with `r1, r2, r3` drawn from a fixed-seed
/// xorshift64\* on `[-8, 8)`, expanded to coefficients — so the analytic answer
/// is known exactly, which makes this an oracle rather than a second opinion.
/// 4 096 equations. Two pass criteria: every `Real`-tagged root satisfies
/// `|a x^3 + b x^2 + c x + d| <= 1e-6` (loose because a cubic's residual is
/// steeply amplified near a triple-ish root), and every returned bit is equal to
/// the per-equation scalar [`CubicEqn::roots`].
///
/// *Result, measured 2026-08-12 by `cubic_batch_matches_analytic_construction`
/// in `parallel/tests.rs`, release build:* worst residual **3.979039e-13** over
/// 12 288 real roots, worst distance from the nearest constructed root
/// **2.079981e-10**; bitwise equality against the scalar solver held on every
/// lane, and (per `poly_batch_bitwise_identical_across_backends`) across
/// `Serial` / `CpuMulti` at 1, 2, 4 and 8 worker threads.
///
/// *Interpretation:* both figures belong to the scalar Cardano solver, not to
/// the batching — the bitwise-equality half of the test proves the batch adds
/// exactly zero error. That the worst *root displacement* (2.1e-10) is three
/// orders larger than the worst *residual* (4.0e-13) is the signature of an
/// ill-conditioned root, not of a bad solve: with roots drawn at random from
/// `[-8, 8)`, some triples come out nearly coincident, and near a near-double
/// root the cubic is flat, so a tiny residual still corresponds to a visibly
/// displaced abscissa. A caller solving a cubic equation of state should expect
/// the same behaviour near the critical point and judge accuracy by the
/// residual.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::parallel::cubic_roots_batch;
/// use outram_foam_basic_lib::polynomial::{CubicEqn, RootType};
///
/// // (x-1)(x-2)(x-3) = x^3 - 6x^2 + 11x - 6
/// let eqns = [CubicEqn::new(1.0, -6.0, 11.0, -6.0)];
/// let roots = cubic_roots_batch(&eqns, ComputeBackend::CpuMulti);
///
/// let mut real: Vec<f64> = (0..3)
///     .filter(|&k| roots[0].root_type(k) == RootType::Real)
///     .map(|k| roots[0].get(k))
///     .collect();
/// real.sort_by(f64::total_cmp);
/// assert_eq!(real.len(), 3);
/// assert!((real[0] - 1.0).abs() < 1e-9);
/// assert!((real[2] - 3.0).abs() < 1e-9);
/// ```
#[must_use]
pub fn cubic_roots_batch(eqns: &[CubicEqn], backend: ComputeBackend) -> Vec<Roots<3>> {
    cubic_roots_batch_min(eqns, backend, POLY_ROOTS_MIN_EQUATIONS)
}

/// [`cubic_roots_batch`] with a caller-supplied size floor; see
/// [`solve_bracketed_batch_min`] for why the `_min` variants exist.
pub(crate) fn cubic_roots_batch_min(
    eqns: &[CubicEqn],
    backend: ComputeBackend,
    min_equations: usize,
) -> Vec<Roots<3>> {
    match effective_backend(backend, eqns.len(), min_equations) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => eqns
            .par_iter()
            .with_min_len(POLY_BLOCK)
            .map(CubicEqn::roots)
            .collect(),
        _ => eqns.iter().map(CubicEqn::roots).collect(),
    }
}
