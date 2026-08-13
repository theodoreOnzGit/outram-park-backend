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

//! Batched derivative-free 1-D **golden-section** extremum search on the hybrid
//! execution backend — contract `N` independent brackets at once, serially or
//! across CPU cores.
//!
//! # Provenance — this is a generalisation, not a new algorithm
//!
//! The algorithm, its contraction constant, its bracket-width stopping rule and
//! its literature citation are taken from the **working, in-production**
//! golden-section search already in this workspace:
//!
//! ```text
//! crates/tampines-steam-tables/src/steam_turbine_equations/
//!     converging_diverging_nozzles/choked_flow/
//!     stagnation_point_outside_vle_ph_dome_multiphase.rs:67
//!         fn golden_section_max_g(...)
//! ```
//!
//! That function locates the homogeneous-equilibrium choked-flow energy-balance
//! maximum `G(p) = rho(p,s0) * sqrt(2*(h0 - h(p,s0)))` along an isentrope and is
//! exercised by that crate's Marviken critical-flow tests. It is cited to:
//!
//! > Price, C. J., & Robertson, B. L. (2012). *Golden Section Search.* In
//! > Encyclopedia of Engineering Optimization and Heuristics (pp. 1-4).
//! > Singapore: Springer Nature Singapore.
//!
//! This module carries the same citation, because it is the same method. What is
//! added here is batching across [`ComputeBackend`], per-lane status reporting,
//! absolute+relative tolerances in place of a hard-coded 1 Pa bracket width,
//! non-finite handling, and the [`Sense`] switch so one kernel serves both
//! minimisation and the maximisation the steam-tables caller actually wants.
//!
//! **One deliberate algorithmic deviation from the original, and it is a
//! speed-up, not a behaviour change in exact arithmetic:** the original
//! re-evaluates *both* interior probes on every iteration. Golden section exists
//! precisely so that one probe of the contracted bracket coincides with a probe
//! already evaluated, costing **one** objective evaluation per iteration rather
//! than two — the original's own comment claims that property while its code
//! does not implement it. This module reuses the retained probe. Consequences:
//!
//! - Roughly **half** the objective evaluations for the same bracket sequence,
//!   which matters when the objective is an IF97 flash.
//! - The retained probe is *reused* rather than *recomputed*, and
//!   `a + gr*(b - a)` recomputed is not bit-identical to the value it replaces.
//!   So this module's iterates can differ from the original's in the last bits.
//!   The bracket sequence, the convergence behaviour and the located extremum to
//!   within the requested tolerance are unchanged.
//! - **A measured, quantified cost, recorded here rather than discovered later.**
//!   The retained probe carries an absolute rounding error fixed at the scale of
//!   the *older, wider* bracket, while the bracket itself keeps shrinking — so
//!   the width's *relative* deviation from the closed form `W0 * gr^k` grows
//!   roughly geometrically with iteration count. Measured by
//!   `bracket_contracts_at_the_golden_ratio` (release, 2026-08-13, `W0 = 10`):
//!   `0` at `k = 1`, `2.675637e-14` at `k = 20`, `2.251199e-11` at `k = 40`,
//!   `2.334157e-6` at `k = 80`. Recomputing both probes — the original's form —
//!   holds the deviation at `<= 6.7e-16` for every one of those `k`.
//!
//!   **This has no effect on the accuracy of the answer**, and it is worth being
//!   precise about why rather than waving it away: at `k = 80` the bracket width
//!   is `1.9e-16`, so a `2.3e-6` *relative* deviation is `4e-22` *absolute* —
//!   far below the `sqrt(eps)`-scale accuracy the located extremum can have in
//!   the first place, and 80 iterations is roughly twice what
//!   [`MinSettings::default`] ever reaches. What it does mean is that the closed
//!   form `W0 * gr^k` stops being an exact predictor of
//!   [`MinSolution::bracket_width`] at large `k`. If you need the width to be
//!   exact rather than the evaluation count to be halved, recompute both probes.
//!
//! # Golden section finds a minimum of a UNIMODAL function — read this first
//!
//! **On a bracket where the objective is not unimodal, golden section returns
//! *a* local extremum, silently, with `Converged` status and no warning of any
//! kind.** It cannot do better: it only ever compares two interior probes, so a
//! second basin outside the surviving sub-bracket is discarded on the first
//! iteration and can never be recovered. There is no test this module could run
//! to detect the situation without evaluating the objective densely, which would
//! defeat the point of using golden section at all.
//!
//! This is the single easiest way to get a confident wrong answer out of this
//! module. If you do not *know* your objective is unimodal on the bracket —
//! from the physics, not from a plot of one case — then coarse-scan first and
//! hand golden section a bracket around the basin you want. That is exactly what
//! the steam-tables `dome_crossing_interior_choke` caller does: a 1500-point
//! scan finds the right basin, and golden section only refines it.
//!
//! # Convergence is on the BRACKET, never on the value
//!
//! [`MinSettings`] deliberately has **no `f_tol` field**, unlike its sibling
//! [`crate::math::parallel::RootSettings`]. A value-based criterion such as
//! `|f(b) - f(a)| <= f_tol` is actively wrong for minimisation, and the reason is
//! the defining property of a minimum rather than an implementation detail: near
//! a smooth minimum `x*`,
//!
//! ```text
//! f(x* + d) - f(x*) ~= 0.5 * f''(x*) * d^2
//! ```
//!
//! so the *values* agree to second order long before the *arguments* do. A
//! function-value test therefore reports success while the abscissa is still
//! wrong in its most significant digits. This module tests the bracket width and
//! nothing else.
//!
//! # Achievable accuracy is about `sqrt(eps)`, not `eps`
//!
//! The same quadratic flatness bounds how well *any* derivative-free method can
//! locate a smooth minimum. Two probes a distance `d` apart around `x*` differ in
//! value by `~0.5*f''*d^2`; once that difference falls below the rounding noise
//! in the *evaluated* `f`, which is `~eps*|f(x*)|`, the comparison that drives
//! the contraction is deciding on noise. Equating the two gives
//!
//! ```text
//! |x_located - x*|  ~  sqrt( 2 * eps * |f(x*)| / |f''(x*)| )
//! ```
//!
//! and with `f` and `f''` both order unity that is the classical floor
//! `sqrt(eps) ~ 1.5e-8` — see [`SQRT_EPSILON`]. This surprises people who expect
//! the `1e-15`-ish accuracy a root finder gets on the same machine, and it is why
//! [`MinSettings::default`] sets `x_tol_rel` to [`SQRT_EPSILON`] rather than the
//! `1e-12` [`crate::math::parallel::RootSettings`] uses. Asking for a tighter
//! tolerance is not rejected — it simply spends ~30 more iterations narrowing a
//! bracket whose midpoint is no more accurate.
//!
//! **Read the formula, not just the headline `sqrt(eps)`.** The floor is set by
//! the objective's *relative precision near its own extremum*, not by golden
//! section. Two consequences, both measured by
//! `flat_minimum_exposes_the_sqrt_eps_limit` in `minimise/tests.rs` on 64 lanes
//! with the tolerance driven below every arithmetic floor (release, 2026-08-13):
//!
//! | Objective | Predicted floor | Measured worst `\|x - x0\|` |
//! |---|---|---|
//! | `1 + (x - x0)^2` — order-unity value | `sqrt(eps) = 1.490116e-8` | **1.053671e-8** |
//! | `1 + (x - x0)^4` — flatter than quadratic | `eps^(1/4) = 1.220703e-4` | **1.026485e-4** |
//! | `(x - x0)^2` — value is exactly `0` | *none of the above* | **8.881784e-16** |
//!
//! The first two rows land just under their predicted floors, so the theory is a
//! usable bound rather than a story. The **flat objective is 9 742x worse** than
//! the quadratic one on identical lanes, brackets and settings — that is the
//! penalty for a minimum that is flatter than quadratic, and every one of those
//! lanes still reported `Converged` with a bracket width that looked tight.
//!
//! The third row is the one that stops `sqrt(eps)` being memorised as a universal
//! constant: an objective whose minimum value is exactly zero, evaluated so that
//! its relative precision survives (`d*d` is exact to a half-ULP in `d`), has no
//! order-unity baseline to be swamped by and reaches **8.9e-16**, seven orders
//! better than the "floor". A physical objective — a mass flux, a residual with a
//! floor, an enthalpy — is the first row, not the third.
//!
//! # Hybrid means dispatch, not two APIs
//!
//! [`golden_section_batch`] takes a [`ComputeBackend`] parameter. There is no
//! `golden_section_batch_parallel()` sibling. With the `parallel` feature off,
//! [`ComputeBackend::CpuMulti`] resolves down to [`ComputeBackend::Serial`]
//! through [`ComputeBackend::resolve`] and the answer is unchanged — bit for
//! bit, not merely close. There is no `Gpu` kernel here yet, so a `Gpu` request
//! degrades to the best available CPU path.
//!
//! # Determinism — bitwise identical across backends and thread counts
//!
//! **This module returns bit-for-bit identical output on
//! [`ComputeBackend::Serial`] and [`ComputeBackend::CpuMulti`], at any thread
//! count, on every run**, provided the caller's objective is a deterministic pure
//! function of `(index, x)`.
//!
//! The argument is the same one [`crate::math::parallel`] makes and it is
//! stronger here than for a reduction: lane `i`'s answer is a pure function of
//! lane `i`'s bracket and objective evaluations, and **no arithmetic crosses
//! lanes at all**. A parallel sum has to re-associate `a + b + c` and
//! floating-point addition is not associative; a batch of independent
//! contractions has nothing to re-associate. Both backends call the same
//! `#[inline]` per-lane kernel; only the identity of the calling thread differs.
//!
//! Verified by the `bitwise_*` tests in `minimise/tests.rs`, which compare serial
//! against `rayon` pools of 1, 2, 4 and 8 workers on 2 048 lanes deliberately
//! built with wildly varying per-lane iteration counts. **Measured 2026-08-13
//! (release, `--features parallel`, 4 logical cores): bit-identical on every
//! observable field of every lane, at all four thread counts, for both
//! [`Sense`] variants.**
//!
//! The `#[ignore]`d `minimise_batch_thread_scaling_benchmark` re-asserts the same
//! identity while timing it, on 65 536 imbalanced lanes, best of 7, with a second
//! independent run alongside:
//!
//! | Worker threads | Time | Speed-up | (repeat) | Bitwise vs serial |
//! |---|---|---|---|---|
//! | *serial reference* | 17883.35 us | 1.00x | 1.00x | — |
//! | 1 | 16192.92 us | 1.10x | 1.07x | identical |
//! | 2 | 10206.05 us | 1.75x | 1.52x | identical |
//! | 4 | 11428.69 us | 1.56x | 1.54x | identical |
//! | 8 | 5708.41 us | 3.13x | 2.31x | identical |
//!
//! The "identical" column is the determinism claim measured rather than argued,
//! and it is asserted by the benchmark itself, not merely printed. **The timings
//! are noisy and should not be read as a scaling study** — the 4-thread row being
//! slower than the 2-thread row in run 1 is not a real effect, and the machine
//! was not idle (a concurrent build was running in another checkout). One
//! machine, four logical cores, one batch, two runs; nothing measured on Android
//! hardware or on a many-core server.
//!
//! The one way a caller can break this is to supply an objective that is not a
//! pure function — one that reads a random number generator, accumulates into
//! shared interior-mutable state, or depends on the calling thread. The `Sync`
//! bound permits it; this contract forbids it.
//!
//! # Non-convergence is reported, never swallowed
//!
//! - Every lane carries a [`MinStatus`], reachable through [`MinBatch::solutions`].
//! - [`MinSolution::extremum`] returns `Option<f64>` — `Some(x)` **only** when
//!   that lane converged. The diagnostic iterate is behind the
//!   deliberately-named [`MinSolution::last_iterate`].
//! - [`MinBatch::extrema`] is the all-or-nothing path: it returns
//!   `Err(`[`MinBatchFailure`]`)` naming the failure count and the first failing
//!   lane, rather than a plausible-looking `Vec<f64>`.
//!
//! **A non-converged lane is never handed back as a bracket endpoint dressed up
//! as a minimum.** A lane that ran out of iterations reports its current bracket
//! midpoint with [`MinStatus::MaxIterations`], which [`MinSolution::extremum`]
//! still refuses to return; a lane with a non-finite bracket end reports
//! [`MinStatus::InvalidBracket`] and a `NaN` iterate, because there is no honest
//! number to give.
//!
//! # What is deliberately NOT here: a Brent parabolic-interpolation minimiser
//!
//! Bead `op-yvj.4.3` offers one as an optional CPU-only faster path. It is not
//! implemented here, on purpose, because **this workspace already contains two
//! of them** and a third would be the exact drift defect the workspace's
//! "Search before building" rule exists to prevent:
//!
//! | Existing implementation | Provenance |
//! |---|---|
//! | `outram-park-fork-dwsim-libs::petroleum::fitting::brent_minimize` | independent implementation from Brent (1973) ch. 5, standing in for DWSIM's `MathEx.BrentOpt.BrentMinimize` |
//! | `outram-park-fork-dwsim-libs::columns::shortcut::brent_minimize` (private) | faithful port of DWSIM `BrentMinimize.vb:176-295` |
//!
//! Both are `f64`-scalar, single-problem, and neither is batched — so there is a
//! real case for a batched Brent minimiser eventually living *here*, since
//! `outram-foam-basic-lib` sits below `dwsim-libs` and the dependency could only
//! ever flow this way. That is a maintainer decision about which of three
//! implementations becomes canonical, not something to settle by quietly adding
//! a fourth. Brent is also explicitly excluded from the GPU by the bead: its
//! branching destroys the lockstep property that makes the batched golden
//! section worth dispatching in the first place.
//!
//! # Units
//!
//! Everything here is dimensionless `f64`, for the same reason
//! [`crate::math::parallel`] is: a generic minimiser has no single physical
//! dimension. One lane's abscissa may be a pressure in pascals and another's a
//! blend factor, and the objective's dimension is whatever the caller returns.
//!
//! `uom` typing is **not stripped** to get here — it is applied at the boundary,
//! by the caller, exactly as the hybrid-backend epic requires: convert into the
//! batch, convert back out. The doctest on [`golden_section_batch`] shows that
//! boundary explicitly, maximising a `uom`-typed mass flux over a `uom`-typed
//! pressure bracket in the shape of the steam-tables choked-flow caller.
//!
//! # Cargo features and portability
//!
//! The `rayon` paths sit behind the crate's `parallel` feature, which is **off by
//! default**; with it off this module still compiles and every entry point still
//! works. `rayon` is pure Rust with no system component, so everything here
//! compiles and runs on `aarch64-linux-android` / Termux exactly as on desktop.
//! Nothing in this module is target-gated.
//!
//! # Example
//!
//! ```rust
//! use outram_foam_basic_lib::compute::ComputeBackend;
//! use outram_foam_basic_lib::math::minimise::{
//!     golden_section_batch, MinProblem, MinSettings, Sense,
//! };
//!
//! // 4 independent parabolas (x - x0)^2, each bracketed on [-5, 5].
//! let vertices = [-2.0, -0.5, 1.25, 3.0];
//! let problems: Vec<MinProblem> = (0..4).map(|_| MinProblem::new(-5.0, 5.0)).collect();
//!
//! let batch = golden_section_batch(
//!     &problems,
//!     Sense::Minimise,
//!     MinSettings::default(),
//!     ComputeBackend::CpuMulti,
//!     |i, x| (x - vertices[i]) * (x - vertices[i]),
//! );
//!
//! let located = batch.extrema().expect("all four lanes converge");
//! for (got, want) in located.iter().zip(vertices) {
//!     // sqrt(eps)-scale accuracy is the floor for a smooth minimum.
//!     assert!((got - want).abs() < 1e-7, "got {got}, want {want}");
//! }
//!
//! // Asking for multi-CPU gives a bit-for-bit identical answer, whether or not
//! // the `parallel` feature is compiled in.
//! let serial = golden_section_batch(
//!     &problems,
//!     Sense::Minimise,
//!     MinSettings::default(),
//!     ComputeBackend::Serial,
//!     |i, x| (x - vertices[i]) * (x - vertices[i]),
//! );
//! assert_eq!(located, serial.extrema().unwrap());
//! ```

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::compute::ComputeBackend;

#[cfg(test)]
mod tests;

// ── Constants ────────────────────────────────────────────────────────────────

/// The golden-section contraction factor `(sqrt(5) - 1) / 2 = 0.6180339...`.
///
/// Each iteration replaces the bracket `[a, b]` by one of `[a, d]` or `[c, b]`,
/// both of width exactly `GOLDEN_RATIO * (b - a)`. That the two candidate
/// sub-brackets have the *same* width is what makes the contraction rate
/// independent of the objective, and the specific value is what makes one probe
/// of the new bracket coincide with a probe of the old one — so each iteration
/// after the first costs a single objective evaluation.
///
/// Equivalently `1 - GOLDEN_RATIO = (3 - sqrt(5)) / 2 = 0.3819660...`, which is
/// the form bead `op-yvj.4.3` and most textbooks quote; the two are the same
/// algorithm written from opposite ends of the bracket.
///
/// # Units
///
/// Dimensionless.
pub const GOLDEN_RATIO: f64 = 0.618_033_988_749_894_9;

/// `sqrt(f64::EPSILON)` = `1.4901161193847656e-8` — the practical accuracy floor
/// for locating a smooth minimum by comparing function values.
///
/// See the module-level "Achievable accuracy" section for the derivation. This
/// is the default [`MinSettings::x_tol_rel`], and it is a *floor*, not a
/// guarantee: an objective flatter than quadratic at its minimum does worse, and
/// an objective evaluated with more than rounding-level noise does worse again.
///
/// # Units
///
/// Dimensionless — it multiplies `|x|` to give an abscissa tolerance.
pub const SQRT_EPSILON: f64 = 1.490_116_119_384_765_6e-8;

/// Problem count below which a [`ComputeBackend::CpuMulti`] request runs
/// [`golden_section_batch`] on the calling thread instead.
///
/// # Measured crossover
///
/// *Methodology.* Measured 2026-08-13 on this workspace's development machine,
/// `std::thread::available_parallelism()` = **4**, release build, `--features
/// parallel`, `rayon`'s global pool, otherwise-idle machine. Batches of `n`
/// independent problems `(x - x0_i)^2` with the vertices `x0_i` spread over
/// `[-3, 3)` so per-lane iteration counts differ, bracket `[-5, 5]`,
/// [`MinSettings::default`], [`Sense::Minimise`]. Best of 7 samples per point,
/// reported as wall clock for one whole batch. Produced by the `#[ignore]`d
/// `minimise_batch_crossover_benchmark` test in `minimise/tests.rs` and
/// transcribed from its printed output. The `cheap` objective is the two-flop
/// parabola above; the `costly` objective adds a `ln`/`exp`/`sqrt` chain per
/// evaluation, standing in for an equation-of-state flash. The `(repeat)`
/// columns are the speed-ups from a second, independent run of the same
/// benchmark, carried alongside rather than averaged away because the parallel
/// column is far noisier than the serial one.
///
/// | Problems | cheap serial | cheap multi | speed-up | (repeat) | costly serial | costly multi | speed-up | (repeat) |
/// |---|---|---|---|---|---|---|---|---|
/// | 16 | 2.24 us | 10.06 us | 0.22x | 0.24x | 13.87 us | 12.94 us | 1.07x | 0.41x |
/// | 32 | 4.40 us | 29.98 us | 0.15x | 0.13x | 29.80 us | 44.04 us | 0.68x | 0.74x |
/// | 64 | 8.77 us | 31.47 us | 0.28x | 0.34x | 63.53 us | 64.86 us | 0.98x | 1.24x |
/// | 128 | 17.96 us | 36.40 us | 0.49x | 0.86x | 140.68 us | 73.43 us | 1.92x | 1.42x |
/// | 256 | 55.42 us | 49.41 us | 1.12x | 1.17x | 290.00 us | 186.77 us | 1.55x | 2.91x |
/// | 512 | 148.21 us | 85.96 us | 1.72x | 3.00x | 613.95 us | 360.42 us | 1.70x | 2.94x |
/// | 1 024 | 370.80 us | 213.96 us | 1.73x | 2.81x | 1255.20 us | 675.41 us | 1.86x | 2.82x |
/// | 4 096 | 1542.64 us | 466.86 us | 3.30x | 3.23x | 5120.51 us | 1711.31 us | 2.99x | 2.86x |
/// | 16 384 | 6260.28 us | 1964.94 us | 3.19x | 2.09x | 20732.41 us | 10689.59 us | 1.94x | 2.69x |
/// | 65 536 | 28254.43 us | 8464.29 us | 3.34x | 1.55x | 84607.33 us | 32110.15 us | 2.63x | 2.03x |
///
/// *Result.* **256** is the smallest size at which *neither* objective lost in
/// *either* run — cheap 1.12x and 1.17x, costly 1.55x and 2.91x — and it is the
/// value this constant takes. At 128 the cheap objective lost both runs (0.49x,
/// 0.86x) while the costly one won both (1.92x, 1.42x), which is the expected
/// shape: a more expensive objective crosses over earlier because there is more
/// work per lane to amortise the same dispatch cost. The honest headline is
/// therefore that **the crossover depends on the caller's objective**, and this
/// single number is set for the cheapest objective a caller is likely to pass, so
/// that it is safe for both.
///
/// # Relationship to [`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`]
///
/// That constant documents itself as an unmeasured placeholder and invites
/// per-kernel overrides. This is that override for the batched golden section.
///
/// *Interpretation.* It lands on exactly the same value as
/// [`crate::math::parallel::ROOT_BATCH_MIN_PROBLEMS`] (256), 16x below the
/// crate-wide placeholder (4 096) and 512x below
/// [`crate::fields::parallel::FIELD_PARALLEL_CROSSOVER`] (131 072). Two kernels
/// agreeing is not a coincidence and is worth stating for whoever takes bead
/// `op-yvj.4.7`: both do tens of iterations of arithmetic per lane against a
/// handful of loaded bytes, so both are compute bound, where the field kernels do
/// one or two flops per element loaded and are memory-bandwidth bound. The
/// crossover tracks that distinction, not the algorithm.
///
/// # Limitations
///
/// One machine, four logical cores, one objective family, idle. Not measured on
/// Android/Termux hardware and not on a many-core server. As with
/// [`crate::math::parallel::ROOT_BATCH_MIN_PROBLEMS`], the crossover moves with
/// the caller's objective cost, so a caller that knows its own objective is
/// expensive is better off naming a [`ComputeBackend`] explicitly than trusting
/// a single number.
///
/// # Units
///
/// A count of independent minimisation problems, dimensionless.
pub const MINIMISE_BATCH_MIN_PROBLEMS: usize = 256;

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

/// The [`ComputeBackend`] [`golden_section_batch`] would actually use for `n`
/// problems if asked for `requested` — without running anything.
///
/// Applies exactly the same reduction the kernel does (feature availability, no
/// GPU kernel here, and the [`MINIMISE_BATCH_MIN_PROBLEMS`] size floor), so what
/// it reports is what would run. Useful for logging and for benchmark harnesses.
///
/// # Arguments
///
/// - `requested` — the backend a caller would pass to [`golden_section_batch`].
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
/// use outram_foam_basic_lib::math::minimise::{
///     minimise_backend_for, MINIMISE_BATCH_MIN_PROBLEMS,
/// };
///
/// // Too small to thread, whatever was asked for.
/// assert_eq!(minimise_backend_for(ComputeBackend::CpuMulti, 8), ComputeBackend::Serial);
///
/// // Big enough; the answer now depends only on whether `parallel` is compiled in.
/// let picked = minimise_backend_for(ComputeBackend::CpuMulti, MINIMISE_BATCH_MIN_PROBLEMS);
/// assert!(picked.is_available());
/// ```
#[must_use]
pub fn minimise_backend_for(requested: ComputeBackend, n: usize) -> ComputeBackend {
    effective_backend(requested, n, MINIMISE_BATCH_MIN_PROBLEMS)
}

// ── Problem, settings, sense ─────────────────────────────────────────────────

/// Whether [`golden_section_batch`] should locate a **minimum** or a **maximum**.
///
/// Golden section is indifferent: it compares two interior probes and keeps the
/// sub-bracket containing the better one, and "better" is the only thing this
/// switch changes. Nothing is negated internally, so the values a solution
/// reports are the caller's own objective values with their own sign.
///
/// [`Maximise`](Self::Maximise) exists because the workspace's production
/// golden-section caller — the homogeneous-equilibrium choked-flow solver in
/// `tampines-steam-tables` — maximises a mass flux. A module that only minimised
/// would force that caller to negate at the boundary, which is exactly the kind
/// of avoidable sign bug a `uom`-typed codebase is trying to design out.
///
/// # Units
///
/// Dimensionless — a mode selector, not a quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Sense {
    /// Locate the abscissa where the objective is smallest. The default.
    #[default]
    Minimise,
    /// Locate the abscissa where the objective is largest.
    Maximise,
}

impl Sense {
    /// Whether `candidate` is the better of two objective values under this sense.
    ///
    /// Strict comparison, so a tie resolves the same way on every run and on
    /// every backend: the incumbent is kept. That matters more than it looks —
    /// a plateau in the objective is common (a guard clause returning `0.0`, a
    /// clamped property table) and a non-strict comparison would make the
    /// surviving bracket depend on evaluation order.
    ///
    /// # Arguments
    ///
    /// - `candidate`, `incumbent` — objective values, in the objective's units.
    ///   Both must be finite; the kernel rejects non-finite values before
    ///   reaching here.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::math::minimise::Sense;
    ///
    /// assert!(Sense::Minimise.is_better(1.0, 2.0));
    /// assert!(Sense::Maximise.is_better(2.0, 1.0));
    /// assert!(!Sense::Minimise.is_better(1.0, 1.0)); // a tie is not better
    /// ```
    #[inline]
    #[must_use]
    pub fn is_better(self, candidate: f64, incumbent: f64) -> bool {
        match self {
            Self::Minimise => candidate < incumbent,
            Self::Maximise => candidate > incumbent,
        }
    }

    /// A short human-readable label, for log lines and failure reports.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Minimise => "minimise",
            Self::Maximise => "maximise",
        }
    }
}

/// One bracketed 1-D extremum problem: an interval believed to contain a single
/// interior extremum of the objective.
///
/// # Fields and valid ranges
///
/// - `lo`, `hi` — the bracket. Both must be finite, or the lane reports
///   [`MinStatus::InvalidBracket`]. They may be given in either order; the kernel
///   normalises, exactly as [`crate::math::parallel::RootProblem`] does. A
///   degenerate bracket (`lo == hi`) converges immediately at that point with
///   zero iterations, which is truthful: a zero-width bracket has already met any
///   non-negative tolerance.
///
/// # The precondition golden section cannot check
///
/// The objective must be **unimodal** on `[lo, hi]` — one interior extremum, no
/// second basin. This is a precondition on the caller, not something the kernel
/// verifies; see the module-level warning. Note that a *monotone* objective on
/// the bracket is a benign special case: the contraction walks to the appropriate
/// endpoint and converges there, which is the intended behaviour when the real
/// extremum lies just outside. It is the *multimodal* case that is dangerous,
/// because the answer looks exactly like a correct one.
///
/// There is no `guess` field, unlike [`crate::math::parallel::RootProblem`].
/// Golden section is driven entirely by the bracket and has nowhere to put a
/// starting iterate; a method that could use one (Brent's parabolic
/// interpolation) is deliberately not implemented here — again, see the module
/// docs.
///
/// # Units
///
/// Dimensionless `f64` in whatever units the caller's abscissa carries — pascals
/// for a choke-pressure search, dimensionless for a blend factor. See the
/// module-level "Units" section.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::math::minimise::MinProblem;
///
/// let p = MinProblem::new(0.0, 1.0);
/// assert_eq!(p.width(), 1.0);
///
/// // Either order is accepted; the width is still positive.
/// let q = MinProblem::new(1.0, 0.0);
/// assert_eq!(q.width(), 1.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinProblem {
    /// One end of the bracket. Finite; need not be less than `hi`.
    pub lo: f64,
    /// The other end of the bracket. Finite; need not be greater than `lo`.
    pub hi: f64,
}

impl MinProblem {
    /// A problem bracketed on `[lo, hi]`.
    ///
    /// # Arguments
    ///
    /// - `lo`, `hi` — the bracket ends, finite, in either order.
    #[must_use]
    pub fn new(lo: f64, hi: f64) -> Self {
        Self { lo, hi }
    }

    /// The bracket width `|hi - lo|`, in the abscissa's units.
    ///
    /// `NaN` if either end is `NaN`; infinite if either end is infinite. Both of
    /// those cases are rejected as [`MinStatus::InvalidBracket`] by the kernel.
    #[must_use]
    pub fn width(&self) -> f64 {
        (self.hi - self.lo).abs()
    }
}

/// Convergence tolerance on the **bracket width**, and the iteration cap.
///
/// A lane is converged when the bracket has shrunk to
/// `x_tol_abs + x_tol_rel * |x_mid|`, where `x_mid` is the current bracket
/// midpoint. There is **no function-value tolerance** — see the module-level
/// "Convergence is on the BRACKET, never on the value" section for why one would
/// be actively misleading here.
///
/// # Fields and valid ranges
///
/// - `x_tol_abs` — absolute abscissa tolerance, in the abscissa's own units.
///   Must be `>= 0`. Guards the case `x* ≈ 0`, where a purely relative tolerance
///   can never be met.
/// - `x_tol_rel` — relative abscissa tolerance, dimensionless, `>= 0`. Values
///   below [`SQRT_EPSILON`] are honoured but buy no accuracy on a smooth
///   objective; they only cost iterations.
/// - `max_iterations` — hard cap. On reaching it a lane reports
///   [`MinStatus::MaxIterations`] and its current bracket midpoint, which
///   [`MinSolution::extremum`] still refuses to hand out. Must be `>= 1`.
///
/// A negative tolerance is not rejected; it simply can never be met, and the lane
/// will report [`MinStatus::MaxIterations`]. That is a truthful outcome rather
/// than a silent success.
///
/// # Defaults, and why they differ from [`crate::math::parallel::RootSettings`]
///
/// `x_tol_abs = 1e-12`, `x_tol_rel = ` [`SQRT_EPSILON`] `= 1.4901161193847656e-8`,
/// `max_iterations = 200`.
///
/// The relative tolerance is ~10 000x looser than the root finder's `1e-12`, and
/// that is not a lowered standard — it is the accuracy a derivative-free
/// minimiser can actually deliver on a smooth objective (module docs,
/// "Achievable accuracy"). Defaulting to a far tighter tolerance would spend
/// about **30 extra iterations** per lane narrowing a bracket whose midpoint does
/// not improve — measured 2026-08-13: 40-47 iterations at these defaults against
/// 77 at `x_tol_abs = 1e-15, x_tol_rel = 0` on the same 64-lane batch, for a
/// worst abscissa error that stays at `1.05e-8` either way.
///
/// The iteration cap is 200 rather than 100 because golden section contracts by a
/// fixed factor of [`GOLDEN_RATIO`] per iteration — it has no superlinear phase
/// to bail it out. Reaching a relative `1.5e-8` from a bracket `1e14` times wider
/// than the answer costs `ln(1e-14)/ln(0.618) ≈ 67` iterations; 200 leaves room
/// for a caller that tightens the tolerance well past the useful floor.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::math::minimise::{MinSettings, SQRT_EPSILON};
///
/// // Struct-update syntax keeps the defaults you did not mean to change.
/// let s = MinSettings {
///     max_iterations: 500,
///     ..MinSettings::default()
/// };
/// assert_eq!(s.x_tol_rel, SQRT_EPSILON);
/// assert_eq!(s.max_iterations, 500);
///
/// // The steam-tables choked-flow caller's rule -- a 1 Pa bracket -- expressed
/// // as an absolute tolerance with the relative part switched off.
/// let one_pascal = MinSettings { x_tol_abs: 1.0, x_tol_rel: 0.0, max_iterations: 100 };
/// assert_eq!(one_pascal.bracket_tolerance(5.0e6), 1.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinSettings {
    /// Absolute bracket-width tolerance, in the abscissa's units. `>= 0`.
    pub x_tol_abs: f64,
    /// Relative bracket-width tolerance, dimensionless. `>= 0`.
    pub x_tol_rel: f64,
    /// Maximum iterations per lane before reporting
    /// [`MinStatus::MaxIterations`]. `>= 1`.
    pub max_iterations: u32,
}

impl Default for MinSettings {
    fn default() -> Self {
        Self {
            x_tol_abs: 1e-12,
            x_tol_rel: SQRT_EPSILON,
            max_iterations: 200,
        }
    }
}

impl MinSettings {
    /// The bracket-width tolerance at abscissa `x`: `x_tol_abs + x_tol_rel * |x|`.
    ///
    /// # Arguments
    ///
    /// - `x` — the abscissa at which to evaluate the tolerance, in the
    ///   abscissa's units. The kernel passes the current bracket midpoint.
    ///
    /// # Returns
    ///
    /// A width in the abscissa's units.
    #[inline]
    #[must_use]
    pub fn bracket_tolerance(&self, x: f64) -> f64 {
        self.x_tol_abs + self.x_tol_rel * x.abs()
    }
}

// ── Per-lane outcome ─────────────────────────────────────────────────────────

/// How one lane of a batch ended.
///
/// Only [`Converged`](Self::Converged) means the lane located an extremum. Every
/// other variant is a failure a caller must handle; see the module-level
/// "Non-convergence is reported, never swallowed" section for the accessors that
/// make it hard to skip.
///
/// | Variant | `last_iterate` | Meaning |
/// |---|---|---|
/// | [`Converged`](Self::Converged) | the located extremum | the bracket met the tolerance |
/// | [`MaxIterations`](Self::MaxIterations) | current bracket midpoint — **not** an extremum | ran out of iterations |
/// | [`InvalidBracket`](Self::InvalidBracket) | `NaN` | a bracket end is not finite |
/// | [`NotFinite`](Self::NotFinite) | the abscissa where it happened | the objective evaluated to `NaN`/infinity |
///
/// There is deliberately **no** analogue of
/// [`crate::math::parallel::RootStatus::NotBracketed`]. A root finder can check
/// its precondition — a sign change across the bracket is one comparison — and
/// refuse when it fails. A minimiser's precondition is unimodality, which cannot
/// be checked from a finite number of evaluations. Adding a status that is never
/// returned would imply a guarantee this module does not provide.
///
/// # Units
///
/// Dimensionless — a status tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MinStatus {
    /// The bracket shrank to the requested tolerance. Note that this says the
    /// bracket converged, **not** that the objective is unimodal — on a
    /// multimodal bracket a lane converges happily to a local extremum.
    Converged,
    /// `max_iterations` was reached with the bracket still wider than the
    /// tolerance. The reported iterate is the current bracket midpoint and is
    /// **not** claimed to be an extremum.
    MaxIterations,
    /// A bracket end was infinite or `NaN`, so there is no interval to contract.
    InvalidBracket,
    /// The objective evaluated to a non-finite number. Iteration stops
    /// immediately, because continuing would propagate the `NaN` into a
    /// plausible-looking answer — and unlike a root finder there is no sign
    /// structure left to recover from.
    NotFinite,
}

impl MinStatus {
    /// Whether this status means the lane located an extremum.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::math::minimise::MinStatus;
    ///
    /// assert!(MinStatus::Converged.is_converged());
    /// assert!(!MinStatus::MaxIterations.is_converged());
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
            Self::InvalidBracket => "invalid-bracket",
            Self::NotFinite => "not-finite",
        }
    }
}

/// The outcome of a single lane: its status, its located abscissa, the objective
/// value there, the final bracket width and the work it took.
///
/// The fields are private on purpose. The only way to get a number claimed to be
/// an extremum is [`Self::extremum`], which returns `Option<f64>` and hands back
/// `Some` only for a converged lane. The raw iterate is available from
/// [`Self::last_iterate`], whose name is chosen so that using it as an answer is
/// a visible decision in the calling code rather than an accident.
///
/// **Abscissa versus value.** [`Self::extremum`] is the *argument* `x*` — the
/// argmin (or argmax). [`Self::extremal_value`] is the *objective value* `f(x*)`
/// there. Mixing those two up is the classic minimisation bug, so they are named
/// apart rather than both being called "the minimum".
///
/// `Copy`, so it can be read out of a [`MinBatch`] without cloning.
///
/// # Units
///
/// [`Self::extremum`], [`Self::last_iterate`] and [`Self::bracket_width`] are in
/// the abscissa's units; [`Self::extremal_value`] and [`Self::last_value`] are in
/// the objective's units; [`Self::iterations`] is a dimensionless count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinSolution {
    x: f64,
    f_x: f64,
    bracket_width: f64,
    iterations: u32,
    status: MinStatus,
}

impl MinSolution {
    /// The located extremum's **abscissa** `x*`, if this lane converged.
    ///
    /// `None` for every failure status. This is the accessor to reach for; the
    /// `Option` is the point.
    ///
    /// # Returns
    ///
    /// `Some(x)` in the abscissa's units when [`Self::status`] is
    /// [`MinStatus::Converged`], otherwise `None`. The value returned is the
    /// midpoint of the final bracket, so the worst-case distance to the true
    /// extremum of a unimodal objective is half of [`Self::bracket_width`] — but
    /// read the module-level accuracy section before believing a bracket width
    /// below `sqrt(eps) * |x|`.
    #[must_use]
    pub fn extremum(&self) -> Option<f64> {
        if self.status.is_converged() {
            Some(self.x)
        } else {
            None
        }
    }

    /// The objective **value** `f(x*)` at the located extremum, if this lane
    /// converged.
    ///
    /// `None` for every failure status. In the objective's units, with the
    /// caller's own sign — nothing is negated for [`Sense::Maximise`].
    #[must_use]
    pub fn extremal_value(&self) -> Option<f64> {
        if self.status.is_converged() {
            Some(self.f_x)
        } else {
            None
        }
    }

    /// The last abscissa the solver held, converged or not.
    ///
    /// **An extremum only when [`Self::status`] is [`MinStatus::Converged`].**
    /// For [`MinStatus::MaxIterations`] this is the midpoint of the bracket the
    /// lane ran out of iterations on, offered for diagnosis — for deciding
    /// whether the lane was close or wildly off — not as an answer. For
    /// [`MinStatus::InvalidBracket`] it is `NaN`, because there is no honest
    /// number to report.
    #[must_use]
    pub fn last_iterate(&self) -> f64 {
        self.x
    }

    /// The objective value at [`Self::last_iterate`], in the objective's units.
    ///
    /// `NaN` when no meaningful evaluation was reached; for
    /// [`MinStatus::NotFinite`] it is the offending non-finite value itself,
    /// which tells you whether the objective overflowed or produced a `NaN`.
    #[must_use]
    pub fn last_value(&self) -> f64 {
        self.f_x
    }

    /// Width of the final bracket, in the abscissa's units.
    ///
    /// `NaN` when the lane never established a bracket. For a converged lane this
    /// bounds the abscissa error at half its value **for a unimodal objective**;
    /// it says nothing at all if the objective was multimodal.
    #[must_use]
    pub fn bracket_width(&self) -> f64 {
        self.bracket_width
    }

    /// Iterations performed on this lane, dimensionless.
    ///
    /// One iteration contracts the bracket by exactly [`GOLDEN_RATIO`] and costs
    /// **one** objective evaluation, on top of the two interior probes every lane
    /// pays up front and the one final evaluation at the returned midpoint. So a
    /// lane reporting `k` iterations called the objective `k + 3` times, unless
    /// it stopped early on a non-finite value.
    #[must_use]
    pub fn iterations(&self) -> u32 {
        self.iterations
    }

    /// How this lane ended.
    #[must_use]
    pub fn status(&self) -> MinStatus {
        self.status
    }

    /// Whether this lane located an extremum.
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
            status: MinStatus::Converged,
        }
    }

    /// A failed lane, carrying whatever diagnostic information exists.
    #[inline]
    fn failed(status: MinStatus, x: f64, f_x: f64, bracket_width: f64, iterations: u32) -> Self {
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
/// - [`Self::extrema`] — all-or-nothing abscissae. `Ok(Vec<f64>)` only when every
///   lane converged; otherwise `Err(`[`MinBatchFailure`]`)`.
/// - [`Self::extremal_values`] — the same, for the objective values.
/// - [`Self::solutions`] — per-lane, when the caller wants to handle failures
///   individually (widen the bracket, flag the cell, fall back to a scan).
///
/// # Units
///
/// See [`MinSolution`].
#[derive(Debug, Clone, PartialEq)]
pub struct MinBatch {
    solutions: Vec<MinSolution>,
}

impl MinBatch {
    /// Every lane's outcome, in problem order.
    #[must_use]
    pub fn solutions(&self) -> &[MinSolution] {
        &self.solutions
    }

    /// Consume the batch and take the outcomes.
    #[must_use]
    pub fn into_solutions(self) -> Vec<MinSolution> {
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
    pub fn get(&self, i: usize) -> Option<MinSolution> {
        self.solutions.get(i).copied()
    }

    /// Whether every lane converged. Vacuously `true` for an empty batch.
    #[must_use]
    pub fn all_converged(&self) -> bool {
        self.solutions.iter().all(MinSolution::converged)
    }

    /// How many lanes failed to converge, dimensionless.
    #[must_use]
    pub fn failure_count(&self) -> usize {
        self.solutions.iter().filter(|s| !s.converged()).count()
    }

    /// The first failing lane and its outcome, if any.
    ///
    /// The natural thing to print when a batch fails: it names the lane to look
    /// at rather than just the count.
    #[must_use]
    pub fn first_failure(&self) -> Option<(usize, MinSolution)> {
        self.solutions
            .iter()
            .enumerate()
            .find(|(_, s)| !s.converged())
            .map(|(i, s)| (i, *s))
    }

    /// Every failing lane, as `(index, outcome)` pairs.
    ///
    /// Allocates, so prefer [`Self::first_failure`] or [`Self::failure_count`] on
    /// a hot path.
    #[must_use]
    pub fn failures(&self) -> Vec<(usize, MinSolution)> {
        self.solutions
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.converged())
            .map(|(i, s)| (i, *s))
            .collect()
    }

    /// The located **abscissae**, or an error naming the failures — the
    /// all-or-nothing path.
    ///
    /// # Returns
    ///
    /// `Ok(v)` with `v[i]` the extremum of problem `i`, in the abscissa's units,
    /// when every lane converged. Otherwise `Err(`[`MinBatchFailure`]`)` carrying
    /// the failure count and the first failing lane. **No `Vec` of
    /// plausible-looking numbers is ever returned for a batch that contained a
    /// failure.**
    ///
    /// An empty batch returns `Ok(vec![])`.
    ///
    /// # Errors
    ///
    /// [`MinBatchFailure`] when one or more lanes did not converge.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::compute::ComputeBackend;
    /// use outram_foam_basic_lib::math::minimise::{
    ///     golden_section_batch, MinProblem, MinSettings, MinStatus, Sense,
    /// };
    ///
    /// // Lane 1's objective is non-finite on its bracket.
    /// let problems = [MinProblem::new(-1.0, 1.0), MinProblem::new(-1.0, 1.0)];
    /// let batch = golden_section_batch(
    ///     &problems,
    ///     Sense::Minimise,
    ///     MinSettings::default(),
    ///     ComputeBackend::Serial,
    ///     |i, x| if i == 1 { f64::NAN } else { x * x },
    /// );
    ///
    /// let err = batch.extrema().expect_err("lane 1 is not finite");
    /// assert_eq!(err.failure_count, 1);
    /// assert_eq!(err.first_index, 1);
    /// assert_eq!(err.first_status, MinStatus::NotFinite);
    ///
    /// // The converged lane is still individually readable.
    /// assert!(batch.solutions()[0].extremum().is_some());
    /// assert!(batch.solutions()[1].extremum().is_none());
    /// ```
    pub fn extrema(&self) -> Result<Vec<f64>, MinBatchFailure> {
        self.check_all_converged()?;
        Ok(self.solutions.iter().map(|s| s.x).collect())
    }

    /// The objective **values** at the located extrema, or an error naming the
    /// failures.
    ///
    /// The value counterpart of [`Self::extrema`], with identical failure
    /// behaviour. In the objective's units, with the caller's own sign.
    ///
    /// # Errors
    ///
    /// [`MinBatchFailure`] when one or more lanes did not converge.
    pub fn extremal_values(&self) -> Result<Vec<f64>, MinBatchFailure> {
        self.check_all_converged()?;
        Ok(self.solutions.iter().map(|s| s.f_x).collect())
    }

    /// `Err` describing the first failure, if any lane failed.
    fn check_all_converged(&self) -> Result<(), MinBatchFailure> {
        if let Some((i, s)) = self.first_failure() {
            return Err(MinBatchFailure {
                total: self.solutions.len(),
                failure_count: self.failure_count(),
                first_index: i,
                first_status: s.status(),
                first_iterations: s.iterations(),
            });
        }
        Ok(())
    }
}

/// One or more lanes of a [`MinBatch`] did not converge.
///
/// Returned by [`MinBatch::extrema`] and [`MinBatch::extremal_values`]. It names
/// both the scale of the problem (how many of how many) and a specific lane to
/// look at, because "3 of 10 000 lanes failed" is only actionable once you know
/// *which* lane.
///
/// # Units
///
/// All counts and indices are dimensionless.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "{failure_count} of {total} minimisation problems did not converge; \
     first failure at lane {first_index} with status {first_status:?} \
     after {first_iterations} iterations"
)]
pub struct MinBatchFailure {
    /// Number of lanes in the batch.
    pub total: usize,
    /// Number of lanes that did not converge.
    pub failure_count: usize,
    /// Index of the first non-converged lane.
    pub first_index: usize,
    /// Why that lane failed.
    pub first_status: MinStatus,
    /// Iterations that lane performed before giving up.
    pub first_iterations: u32,
}

// ── The public batch entry point ─────────────────────────────────────────────

/// Locate `N` independent 1-D extrema by golden-section search, on the chosen
/// backend.
///
/// This is the entry point for derivative-free bracketed extremum search: an
/// objective you can evaluate on a bracket you believe contains one interior
/// extremum, for many independent lanes at once. It is the right tool when a
/// root find is the wrong one — minimising a residual, picking a limiter blend
/// factor, or locating a property-inversion objective's turning point where that
/// objective is not monotone.
///
/// # Arguments
///
/// - `problems` — one [`MinProblem`] per lane. `problems[i]`'s bracket is used
///   with `f(i, ·)`.
/// - `sense` — [`Sense::Minimise`] or [`Sense::Maximise`]; nothing is negated
///   internally, so reported values keep the caller's sign.
/// - `settings` — bracket tolerance and iteration cap; see [`MinSettings`].
/// - `backend` — requested execution backend. What actually runs is
///   [`minimise_backend_for`] applied to it: an unavailable backend degrades,
///   `Gpu` degrades (no GPU kernel here yet), and a batch below
///   [`MINIMISE_BATCH_MIN_PROBLEMS`] runs serially. **None of those changes the
///   answer.**
/// - `f` — the objective. `f(i, x)` must return the value of lane `i`'s function
///   at abscissa `x`. It **must be a pure deterministic function of its
///   arguments** — see the module-level "Determinism" section for what breaks if
///   it is not. It is called from multiple threads on the `CpuMulti` path, hence
///   the `Sync` bound; the bound is present in both feature builds so that
///   enabling `parallel` never changes a public signature.
///
/// # Returns
///
/// A [`MinBatch`] with one [`MinSolution`] per problem, in problem order. An
/// empty `problems` slice returns an empty batch and calls `f` zero times.
///
/// # Cost
///
/// Two objective evaluations up front, one per iteration, and one final
/// evaluation at the returned bracket midpoint. The iteration count is fixed by
/// the geometry alone — `ceil(ln(tol/W0) / ln(GOLDEN_RATIO))` — and does **not**
/// depend on the objective's shape, which is what makes this kernel's per-lane
/// work uniform and its batched form the cleanest fit for lockstep execution in
/// the whole hybrid-backend epic.
///
/// # Preconditions the kernel cannot check
///
/// The objective must be unimodal on each bracket. It is not verified and cannot
/// be; a multimodal bracket yields *a* local extremum with `Converged` status.
/// See the module-level warning — this is the failure mode to worry about.
///
/// # Determinism
///
/// Bit-for-bit identical on [`ComputeBackend::Serial`] and
/// [`ComputeBackend::CpuMulti`], at any thread count, for a pure `f`. Both
/// backends run the same per-lane kernel and no arithmetic crosses lanes.
///
/// # Non-convergence
///
/// Per lane, never swallowed. A lane whose bracket end is non-finite reports
/// [`MinStatus::InvalidBracket`] and a `NaN` iterate; a lane whose objective
/// returns a non-finite value reports [`MinStatus::NotFinite`] at the offending
/// abscissa; a lane that exhausts `max_iterations` reports
/// [`MinStatus::MaxIterations`] and its bracket midpoint, which
/// [`MinSolution::extremum`] still refuses to hand out. [`MinBatch::extrema`]
/// turns any failure into an `Err`.
///
/// # Verification
///
/// *Methodology.* Checked against extrema known in closed form, so the oracle is
/// exact rather than another implementation. Four families, all on 64 lanes
/// (except where noted) with vertices spread over `[-3, 3)` and bracket `[-5, 5]`:
///
/// 1. **Quadratic** `f(x) = 1 + (x - x0)^2` at [`MinSettings::default`]. Exact
///    minimiser `x0`, minimum value `1`. The realistic reference case — an
///    order-unity value at the extremum, as a physical objective has.
/// 2. **Deliberately flat** `f(x) = 1 + (x - x0)^4`, with the tolerance driven
///    *below* every arithmetic floor so the arithmetic binds rather than the
///    stopping rule. Same exact minimiser; quartically flat there.
/// 3. **Transcendental** `f(x) = x ln x` on `[0.05, 3]`, one lane, whose
///    minimiser is `1/e = 0.36787944117144233` and minimum value `-1/e`, both
///    exactly.
/// 4. **Maximisation** `f(x) = sin x` on `[0, pi]`, one lane, maximiser `pi/2`
///    and maximum `1`, run under [`Sense::Maximise`] so the sense switch is
///    checked against an analytic answer rather than against the minimisation
///    path — a mirror test would pass even if both senses were wrong in the same
///    way.
///
/// *Pass criterion.* `status == Converged` on every lane, and
/// `|x_located - x_analytic| <= 1e-6` for families 1, 3 and 4 (two orders of
/// margin over the `sqrt(eps) ≈ 1.5e-8` floor), plus `|f_located - f_analytic| <=
/// 1e-12` for families 3 and 4. Family 2's criterion is deliberately `1e-2`,
/// because `eps^(1/4) ≈ 1.2e-4` is the floor there and asserting `1e-6` would be
/// asserting something false.
///
/// *Results, measured 2026-08-13 by `golden_section_matches_analytic_minima`,
/// `flat_minimum_exposes_the_sqrt_eps_limit`,
/// `golden_section_matches_analytic_transcendental_minimum` and
/// `maximise_matches_analytic_sine_peak` in `minimise/tests.rs`, release build,
/// 4 logical cores:*
///
/// | Family | worst `\|x - x_analytic\|` | worst value error | iterations |
/// |---|---|---|---|
/// | 1. `1 + (x - x0)^2`, default tol | **1.151892e-8** | **2.220446e-16** | 40-47 |
/// | 2. `1 + (x - x0)^4`, tol below the floor | **1.026485e-4** | — | 77 |
/// | 3. `x ln x` | **6.967008e-10** | **5.551115e-17** | 42 |
/// | 4. `sin x`, [`Sense::Maximise`] | **0.000000e0** | **0.000000e0** | 39 |
///
/// *Interpretation.* Family 1 lands at 0.77x the `sqrt(eps)` floor — the method
/// delivers exactly the accuracy the arithmetic allows and no more, which is what
/// it should do given the default `x_tol_rel` *is* that floor. Its value error is
/// one ULP at `f = 1`, illustrating the module's point about why a value-based
/// convergence test would be wrong: the value is already correct to the last bit
/// while the abscissa is still wrong in its 9th digit. Family 2 is **9 742x
/// worse** in the abscissa than family 1 on identical lanes, which is the flat-
/// minimum penalty measured rather than asserted. Family 3 confirms the same
/// behaviour on a transcendental objective, ruling out an artefact of polynomial
/// test functions. Family 4 recovers `pi/2` and `1.0` exactly, so
/// [`Sense::Maximise`] is not merely self-consistent but correct, and the value
/// is returned with the caller's own positive sign — nothing is negated
/// internally.
///
/// The iteration counts in family 1 vary (40-47) only because
/// [`MinSettings::default`] scales its tolerance with `|x_mid|`; under a purely
/// absolute tolerance the count is identical on every lane, since golden
/// section's contraction is fixed by geometry and not by the objective. That
/// uniformity is verified separately by `bracket_contracts_at_the_golden_ratio`.
///
/// # Example — a batch of parabolas
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::minimise::{
///     golden_section_batch, MinProblem, MinSettings, Sense,
/// };
///
/// // Minimise (x - 2)^2 + 1 on [-10, 10]: the vertex is at x = 2, value 1.
/// let problems = [MinProblem::new(-10.0, 10.0)];
/// let batch = golden_section_batch(
///     &problems,
///     Sense::Minimise,
///     MinSettings::default(),
///     ComputeBackend::Serial,
///     |_, x| (x - 2.0) * (x - 2.0) + 1.0,
/// );
///
/// let s = batch.solutions()[0];
/// assert!(s.converged());
/// // The abscissa and the value are separate accessors, on purpose.
/// assert!((s.extremum().unwrap() - 2.0).abs() < 1e-6);
/// assert!((s.extremal_value().unwrap() - 1.0).abs() < 1e-12);
/// ```
///
/// # Example — maximisation at the `uom` boundary
///
/// The batch is dimensionless and the caller converts at its edge. This lane has
/// the shape of the workspace's production golden-section caller: maximise a
/// mass flux over a pressure bracket, keeping `uom` typing on both sides of the
/// call.
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::minimise::{
///     golden_section_batch, MinProblem, MinSettings, Sense,
/// };
/// use uom::si::f64::{MassFlux, Pressure};
/// use uom::si::mass_flux::kilogram_per_square_meter_second;
/// use uom::si::pressure::pascal;
///
/// // A stand-in for G(p) along an isentrope: a single interior peak at 2 MPa.
/// // (Analytic, not thermodynamic -- the real one is an IF97 flash.)
/// let g_of_p = |p: Pressure| -> MassFlux {
///     let x = p.get::<pascal>() / 1.0e6;
///     MassFlux::new::<kilogram_per_square_meter_second>(1.0e4 * (1.0 - (x - 2.0) * (x - 2.0) / 9.0))
/// };
///
/// // Convert in: the bracket in pascals, as plain f64.
/// let lo = Pressure::new::<pascal>(0.1e6);
/// let hi = Pressure::new::<pascal>(5.0e6);
/// let problems = [MinProblem::new(lo.get::<pascal>(), hi.get::<pascal>())];
///
/// let batch = golden_section_batch(
///     &problems,
///     Sense::Maximise,
///     MinSettings { x_tol_abs: 1.0, x_tol_rel: 0.0, max_iterations: 100 },
///     ComputeBackend::Serial,
///     |_, p_pa| g_of_p(Pressure::new::<pascal>(p_pa)).get::<kilogram_per_square_meter_second>(),
/// );
///
/// // Convert out: back to a typed pressure and mass flux.
/// let s = batch.solutions()[0];
/// let p_star = Pressure::new::<pascal>(s.extremum().expect("converged"));
/// let g_star = MassFlux::new::<kilogram_per_square_meter_second>(
///     s.extremal_value().expect("converged"),
/// );
///
/// assert!((p_star.get::<pascal>() - 2.0e6).abs() <= 1.0); // 1 Pa bracket
/// assert!((g_star.get::<kilogram_per_square_meter_second>() - 1.0e4).abs() < 1.0e-3);
/// ```
#[must_use]
pub fn golden_section_batch<F>(
    problems: &[MinProblem],
    sense: Sense,
    settings: MinSettings,
    backend: ComputeBackend,
    f: F,
) -> MinBatch
where
    F: Fn(usize, f64) -> f64 + Sync,
{
    golden_section_batch_min(
        problems,
        sense,
        settings,
        backend,
        MINIMISE_BATCH_MIN_PROBLEMS,
        f,
    )
}

/// [`golden_section_batch`] with the size floor supplied by the caller.
///
/// Exists so the crossover benchmark can measure the multi-CPU path *below*
/// [`MINIMISE_BATCH_MIN_PROBLEMS`] — the only way to find where the crossover
/// actually is — and so the cross-backend bitwise tests are not vacuous on small
/// batches. Not public: production callers get the measured floor.
pub(crate) fn golden_section_batch_min<F>(
    problems: &[MinProblem],
    sense: Sense,
    settings: MinSettings,
    backend: ComputeBackend,
    min_problems: usize,
    f: F,
) -> MinBatch
where
    F: Fn(usize, f64) -> f64 + Sync,
{
    let n = problems.len();
    let solutions: Vec<MinSolution> = match effective_backend(backend, n, min_problems) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => problems
            .par_iter()
            .enumerate()
            // No `min_len` floor: per-lane iteration counts vary with the ratio
            // of bracket width to tolerance, so the splitter is left free to
            // steal down to a single lane.
            .map(|(i, p)| golden_section_one(i, *p, sense, settings, &f))
            .collect(),
        _ => problems
            .iter()
            .enumerate()
            .map(|(i, p)| golden_section_one(i, *p, sense, settings, &f))
            .collect(),
    };
    MinBatch { solutions }
}

// ── Per-lane kernel — one implementation, both backends ──────────────────────

/// The single-lane golden-section contraction that **both** backends run.
///
/// `#[inline]` so the serial loop and the `rayon` map compile to the same inner
/// code — part of why the two backends agree bit for bit.
///
/// The structure is the one taken from `tampines-steam-tables`'
/// `golden_section_max_g` (see the module docs for the citation), with the
/// retained probe reused rather than recomputed:
///
/// ```text
/// c = b - gr*(b - a);  d = a + gr*(b - a)
/// loop:
///     if (b - a) <= tol: done, answer is (a + b)/2
///     if f(c) is better than f(d):  b <- d;  d <- c;  c <- b - gr*(b - a)  [1 eval]
///     else:                         a <- c;  c <- d;  d <- a + gr*(b - a)  [1 eval]
/// ```
#[inline]
fn golden_section_one<F>(
    i: usize,
    p: MinProblem,
    sense: Sense,
    settings: MinSettings,
    f: &F,
) -> MinSolution
where
    F: Fn(usize, f64) -> f64,
{
    let nan = f64::NAN;
    if !p.lo.is_finite() || !p.hi.is_finite() {
        return MinSolution::failed(MinStatus::InvalidBracket, nan, nan, nan, 0);
    }

    // Normalise the bracket; either order is accepted, as for `RootProblem`.
    let (mut a, mut b) = if p.lo <= p.hi {
        (p.lo, p.hi)
    } else {
        (p.hi, p.lo)
    };

    // The two interior probes, placed so that one survives each contraction.
    let mut c = b - GOLDEN_RATIO * (b - a);
    let mut d = a + GOLDEN_RATIO * (b - a);
    let mut f_c = f(i, c);
    if !f_c.is_finite() {
        return MinSolution::failed(MinStatus::NotFinite, c, f_c, b - a, 0);
    }
    let mut f_d = f(i, d);
    if !f_d.is_finite() {
        return MinSolution::failed(MinStatus::NotFinite, d, f_d, b - a, 0);
    }

    for iteration in 1..=settings.max_iterations {
        let width = b - a;
        let mid = a + 0.5 * width;
        if width <= settings.bracket_tolerance(mid) {
            let f_mid = f(i, mid);
            if !f_mid.is_finite() {
                return MinSolution::failed(MinStatus::NotFinite, mid, f_mid, width, iteration - 1);
            }
            return MinSolution::converged_at(mid, f_mid, width, iteration - 1);
        }

        if sense.is_better(f_c, f_d) {
            // The extremum lies in [a, d]: keep `c` as the new right-hand probe.
            b = d;
            d = c;
            f_d = f_c;
            c = b - GOLDEN_RATIO * (b - a);
            f_c = f(i, c);
            if !f_c.is_finite() {
                return MinSolution::failed(MinStatus::NotFinite, c, f_c, b - a, iteration);
            }
        } else {
            // The extremum lies in [c, b]: keep `d` as the new left-hand probe.
            a = c;
            c = d;
            f_c = f_d;
            d = a + GOLDEN_RATIO * (b - a);
            f_d = f(i, d);
            if !f_d.is_finite() {
                return MinSolution::failed(MinStatus::NotFinite, d, f_d, b - a, iteration);
            }
        }
    }

    let width = b - a;
    let mid = a + 0.5 * width;
    MinSolution::failed(
        MinStatus::MaxIterations,
        mid,
        f(i, mid),
        width,
        settings.max_iterations,
    )
}
