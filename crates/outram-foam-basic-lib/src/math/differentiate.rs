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

//! **Numerical differentiation of a supplied function** — finite differences,
//! batched derivatives, and batched Jacobians, dispatched across
//! [`ComputeBackend`].
//!
//! # This is NOT the FV gradient operator
//!
//! [`crate::fv_operators`] implements the *spatial* finite-volume `grad`, `div`
//! and `laplacian` over a mesh: they differentiate a **field** with respect to
//! **position**, using face fluxes and cell volumes, and they are the right tool
//! for a PDE discretisation. This module differentiates an arbitrary
//! **caller-supplied function** with respect to its own arguments, by sampling
//! it. If you are discretising a transport equation you want `fv_operators`; if
//! you need `df/dx` of a closure, a property correlation or an ODE right-hand
//! side, you are in the right place.
//!
//! # The problem this exists to solve
//!
//! [`crate::ode::OdeSystem::jacobian`] has a default body that is
//! `unimplemented!()`. Any system that does not hand-code its Jacobian
//! **panics** the moment [`crate::ode::Rosenbrock23`] — the crate's only stiff
//! solver — asks for one, and inside
//! [`crate::ode::parallel::integrate_ensemble`] that panic propagates out
//! through the `rayon` scope. [`NumericalJacobian`] closes that hole: wrap a
//! system, and `Rosenbrock23` integrates it with no hand-coded Jacobian at all.
//! Measured cost of doing so, on Van der Pol (`mu = 5`, `x` in `[0, 10]`,
//! tolerances `1e-8`) — see "Measured cost against a hand-coded Jacobian" below
//! — is **1.9x** wall clock for the same answer to 8 decimals.
//!
//! # Provenance — a generalisation of two settled workspace conventions
//!
//! Nothing here is a new algorithm. Both halves of the formulation are taken
//! from implementations already working in this workspace, and the divergences
//! are stated rather than left to be discovered.
//!
//! **The Jacobian assembly** generalises:
//!
//! ```text
//! crates/outram-park-fork-dwsim-libs/src/columns/linalg.rs:183
//!     pub fn finite_difference_jacobian<F>(f: &mut F, x: &[f64], epsilon: f64)
//!         -> Option<Array2<f64>>
//! ```
//!
//! itself a port of DWSIM's `NewtonRaphson.vb:669-705` (`FunctionGradient`),
//! used there for the Naphtali-Sandholm column solver's initial Broyden
//! Jacobian. Kept from it: the **central** stencil, the **relative**
//! perturbation, and the **failure-is-`Option`** convention — a caller never
//! receives a matrix it cannot tell apart from a good one. `dwsim-libs` cannot
//! be depended on from here (this crate has no internal workspace dependencies,
//! by policy), so this is a reuse of *formulation*, not of code.
//!
//! Three deliberate divergences from it, each because the alternative is a known
//! defect:
//!
//! | This module | `finite_difference_jacobian` | Why |
//! |---|---|---|
//! | `h = rel * max(\|x\|, min_scale)` | `x*(1±eps)`, or `eps` and `2*eps` when `x == 0` | The `x == 0` branch silently switches to a *one-sided* stencil at a *different* step, so the scheme changes with the data. The `max` floor keeps one scheme everywhere. |
//! | A failed entry is `NaN` and the status says so | a non-finite entry is written as `0.0` | A zero is a *plausible* Jacobian entry. It cannot be detected downstream, and a Newton or Rosenbrock step built on it returns a wrong answer instead of an error. |
//! | Divides by the realised step `xp - xm` | divides by the requested `2*eps*x` | `x + h` is not representable, so the requested step is not the step taken. See [`derivative`]. |
//!
//! **The step-size rule** is the one already in:
//!
//! ```text
//! crates/outram-park-fork-offbeat/src/rheology/aster/integration.rs:298
//!     pub fn newton_perturbed(...)   // h = perturbation * x.abs().max(1.0)
//!     pub fn perturbed_default() -> f64   // f64::EPSILON.cbrt()
//! ```
//!
//! which is upstream Code_Aster's `NEWTON_PERT`. This module adopts both the
//! `max(|x|, 1)` floor and `eps^(1/3)` for the central scheme verbatim — see
//! [`CBRT_EPSILON`] and [`DiffSettings::step_for`] — and extends the same
//! reasoning to the other three orders.
//!
//! A third, narrower precedent —
//! `tampines-steam-tables`' `w_ps_eqm_region4_finite_diff_vol`
//! (`region_4_vap_liq_equilibrium/speed_of_sound_eqm.rs:83`) — takes
//! `dv/dp|_s` by central differences with a hard-coded `dp = 1e-4 * p`, clamped
//! at the minimum table pressure. That is a *relative* step of `1e-4`, sixteen
//! times coarser than [`CBRT_EPSILON`], which is the right call there because
//! the IF97 flash it differentiates is far noisier than machine epsilon. It is
//! recorded here as the standing reminder that **the optimal step assumes the
//! function is evaluated to rounding accuracy**, and a caller whose function is
//! noisier should raise [`DiffSettings::relative_step`] accordingly.
//!
//! # Achievable accuracy is `sqrt(eps)` to `eps^(4/5)`, NEVER `eps`
//!
//! This is the single most common misunderstanding about finite differences, so
//! it is measured rather than asserted. Truncation error falls as `h^p` while
//! round-off grows as `eps/h`; their sum is minimised at `h ~ eps^(1/(p+1))`,
//! where the achievable accuracy is `~ eps^(p/(p+1))`:
//!
//! | Scheme | Order `p` | Optimal `h` | Predicted accuracy |
//! |---|---|---|---|
//! | [`DiffScheme::Forward`] / [`DiffScheme::Backward`] | 1 | `sqrt(eps) = 1.49e-8` | `sqrt(eps) = 1.49e-8` |
//! | [`DiffScheme::Central`] | 2 | `eps^(1/3) = 6.06e-6` | `eps^(2/3) = 3.67e-11` |
//! | [`DiffScheme::Central4th`] | 4 | `eps^(1/5) = 7.40e-4` | `eps^(4/5) = 3.00e-13` |
//!
//! **A caller expecting `1e-15` from a finite difference will be wrong by seven
//! orders of magnitude for a forward difference.** If you need machine
//! precision, hand-code the derivative.
//!
//! *Measured.* `accuracy_floor_at_the_default_step` in `differentiate/tests.rs`,
//! release, 2026-08-13. Worst relative error over six points in `[0.25, 3.3]`,
//! each scheme at its own default step:
//!
//! | Function | forward | backward | central | central-4th |
//! |---|---|---|---|---|
//! | `sin` | 1.281191e-8 | 1.401018e-8 | 6.528067e-11 | 1.785239e-13 |
//! | `exp` | 2.413003e-8 | 2.383524e-8 | 6.640831e-11 | 1.800352e-13 |
//! | `x^3 - 2x` | 4.470348e-8 | 4.470348e-8 | 4.583556e-11 | 9.992007e-14 |
//! | `1/(1 + x^2)` | 8.046627e-9 | 8.430242e-9 | 2.949979e-11 | 9.620083e-14 |
//! | `tanh` | 6.775837e-9 | 8.125324e-9 | 1.244682e-11 | 1.963985e-13 |
//! | **worst** | **4.470348e-8** | **4.470348e-8** | **6.640831e-11** | **1.963985e-13** |
//! | *predicted* | *1.490116e-8* | *1.490116e-8* | *3.666853e-11* | *3.000214e-13* |
//!
//! Every scheme lands within a factor of three of its prediction, and the
//! **ordering is exactly as predicted: central-4th beats central by 338x, and
//! central beats forward by 673x.** The theory is a usable bound, not a story.
//!
//! *Observed convergence order.* `observed_convergence_order_matches_theory`,
//! same run. Absolute error of `d/dx sin(x)` at `x = 1` (exact
//! `5.40302305868139765e-1`) against the relative step:
//!
//! | Relative step | forward | backward | central | central-4th |
//! |---|---|---|---|---|
//! | `1e-1` | 4.293855e-2 | 4.113845e-2 | 9.000537e-4 | 1.125295e-7 |
//! | `1e-2` | 4.216325e-3 | 4.198315e-3 | 9.004993e-6 | 1.126843e-11 |
//! | `1e-3` | 4.208255e-4 | 4.206454e-4 | 9.005042e-8 | 1.049161e-13 |
//! | `1e-4` | 4.207445e-5 | 4.207265e-5 | 9.003700e-10 | 3.312906e-13 |
//! | **observed order** | **1.0079** | **0.9912** | **1.9998** | **3.9994** |
//!
//! The fourth-order column turning back upward at `1e-4` (1.05e-13 to 3.31e-13)
//! is the round-off wall arriving, exactly where `eps^(1/5) = 7.4e-4` says it
//! should.
//!
//! *The round-off wall.* `a_step_far_below_the_optimum_is_worse_not_better`,
//! same run. Central difference, `d/dx sin(x)` at `x = 1`, relative error:
//!
//! | Relative step | Relative error |
//! |---|---|
//! | `1.0000e-2` | 1.666658e-5 |
//! | `6.0555e-6` ([`CBRT_EPSILON`]) | **5.373555e-12** |
//! | `1.0000e-8` | 5.303737e-9 |
//! | `1.0000e-10` | 1.909780e-7 |
//! | `1.0000e-12` | 5.609880e-5 |
//! | `1.0000e-14` | 7.666335e-3 |
//!
//! **A step ten thousand times smaller than the optimum is ten million times
//! worse.** "Make `h` tiny for accuracy" is the intuition this table exists to
//! destroy.
//!
//! # Forward against central: the cost/accuracy trade, measured
//!
//! For an `n`-dimensional Jacobian the evaluation counts are `n + 1`, `2n` and
//! `4n` (see [`DiffScheme::evaluations_per_jacobian`]). The measured accuracy
//! ratio from the table above is **673x** for one extra evaluation per column
//! going forward to central, and a further **338x** for two more going to
//! `Central4th`. Central is very nearly always the right default, which is why
//! it is what [`DiffSettings::central`] exists for and what
//! [`NumericalJacobian`]'s documentation recommends; [`DiffScheme::Forward`] is
//! for the case where the function is genuinely expensive and `1e-8` is enough.
//!
//! # Verification against analytic Jacobians
//!
//! *Methodology.* Three systems whose Jacobians can be written down exactly are
//! differenced and compared entry by entry: a **quadratic** system
//! `[x0^2 + x1, x0*x1^2]`, a **trigonometric** system
//! `[sin(x0)cos(x1), exp(x0)*x1]` in which no derivative of any order vanishes,
//! and a **stiff linear pair** `[-1000*y0 + y1, y0 - y1]` with a 1000:1 entry
//! spread. Pass criterion: worst relative error below `1e-7` for the first-order
//! schemes and `1e-10`/`1e-11` for the higher-order ones. All four schemes pass
//! on all three systems (release, 2026-08-13).
//!
//! *The stiff pair is the informative one*, because a **linear** system has
//! exactly zero truncation error — so every digit lost is cancellation, and the
//! measurement isolates it. Absolute error per entry at `y = [0.4, -0.9]`:
//!
//! | Scheme | `J[0][0]` (-1000) | `J[0][1]` (1) | `J[1][0]` (1) | `J[1][1]` (-1) |
//! |---|---|---|---|---|
//! | forward | 0 | 0 | 0 | 0 |
//! | backward | 0 | 0 | 0 | 0 |
//! | central | 6.600658e-10 | **1.778424e-9** | 9.167112e-12 | 1.833422e-11 |
//! | central-4th | 5.456968e-12 | 6.380474e-11 | 2.498002e-14 | 0 |
//!
//! **`J[0][1]` is the worst entry by two orders of magnitude, and it is the
//! small entry in the row that also holds `-1000`.** Row 0 evaluates to about
//! `-400.9`; perturbing `x1` changes it by `1.2e-5`, so forming that difference
//! discards eleven significant digits before the division. This is a **general
//! property of finite-difference Jacobians, not of this implementation**: an
//! entry is only as accurate as its own magnitude relative to the largest term
//! in its row. A badly-scaled system loses precision in exactly the entries a
//! stiff solver most needs. If that matters, hand-code the Jacobian or rescale
//! the equations.
//!
//! The one-sided schemes returning **exactly zero** error here is a property of
//! this particular linear case (`f(x+h) - f(x)` is exact when `f` is affine and
//! the operands round identically) and must not be read as one-sided differences
//! being more accurate in general — the accuracy table above shows them 673x
//! *worse* on smooth non-linear functions.
//!
//! # Measured cost against a hand-coded Jacobian
//!
//! *Methodology.* Van der Pol `mu = 5`, `y(0) = [2, 0]`, integrated over
//! `x` in `[0, 10]` by [`crate::ode::Rosenbrock23`] with `abs_tol = rel_tol =
//! 1e-8`; best of 5, release, default features, the loaded 4-core machine
//! described on [`DERIVATIVE_BATCH_MIN_POINTS`]. Van der Pol *has* an analytic
//! Jacobian, which is the baseline. Produced by the `#[ignore]`d
//! `numerical_jacobian_overhead_benchmark`, two runs.
//!
//! | Jacobian | Time (A) | Time (B) | vs analytic (A) | (B) | `y0(10)` |
//! |---|---|---|---|---|---|
//! | analytic (hand-coded) | 849.99 us | 855.47 us | 1.00x | 1.00x | -1.15870127 |
//! | [`DiffScheme::Forward`] | 1619.03 us | 1603.88 us | 1.90x | 1.87x | -1.15870127 |
//! | [`DiffScheme::Backward`] | 1607.45 us | 1614.33 us | 1.89x | 1.89x | -1.15870127 |
//! | [`DiffScheme::Central`] | 1673.66 us | 1677.88 us | 1.97x | 1.96x | -1.15870127 |
//! | [`DiffScheme::Central4th`] | 2487.96 us | 2455.24 us | 2.93x | 2.87x | -1.15870127 |
//!
//! **All four schemes reproduce the analytic result to all eight printed
//! decimals**, at roughly twice the cost. That is the honest headline: a
//! numerical Jacobian is not free, it is not exact, and for a `n = 2` stiff
//! system it costs about a factor of two and changes nothing you can see.
//!
//! # A NaN Jacobian is NOT reported by the solver — check the counter
//!
//! When a Jacobian cannot be differenced, this module writes `NaN` into the
//! entries and says so through [`DiffStatus`]. The natural expectation is that
//! `Rosenbrock23` then fails loudly. **It does not, and this was measured
//! rather than assumed** (`a_jacobian_that_cannot_be_differenced_is_counted_and_reaches_the_solver_as_nan`):
//!
//! - [`crate::ode::Rosenbrock23::integrate`] returns **`Ok(())`**;
//! - the state vector comes back **`NaN`**.
//!
//! The cause is in the ODE layer, not here: `ode::normalize_error` folds the
//! per-equation errors with `f64::max`, and `f64::max(0.0, NaN)` is `0.0` — so a
//! `NaN` error estimate looks like a *perfectly converged* step and every
//! sub-step is accepted. Nothing in this module can change that.
//!
//! **Consequence for callers:** [`NumericalJacobian::non_finite_jacobians`] is
//! the only in-band signal that anything went wrong. Check it after any
//! integration whose result you intend to trust. It is not decoration.
//!
//! # Hybrid means dispatch, not two APIs
//!
//! Every entry point takes a [`ComputeBackend`] parameter; there is no
//! `_parallel()` sibling. With the `parallel` feature off,
//! [`ComputeBackend::CpuMulti`] resolves down to [`ComputeBackend::Serial`] and
//! the answer is unchanged — bit for bit, not merely close. There is no `Gpu`
//! kernel here yet, so a `Gpu` request degrades to the best available CPU path.
//!
//! **Two independent parallel axes** live here, with separately measured
//! crossovers 2048x apart:
//!
//! | Entry point | Parallel over | Crossover |
//! |---|---|---|
//! | [`derivative_batch`] | independent points | [`DERIVATIVE_BATCH_MIN_POINTS`] = 65 536 |
//! | [`jacobian_batch`] | independent lanes | [`JACOBIAN_BATCH_MIN_PROBLEMS`] = 256 |
//! | [`jacobian`] | the columns of **one** Jacobian | [`JACOBIAN_COLUMN_MIN_DIMENSION`] = 32 |
//!
//! They are not nested: [`jacobian_batch`] runs each lane's columns serially,
//! because the lane axis is already saturating the pool.
//!
//! # Determinism — bitwise identical across backends and thread counts
//!
//! **This module returns bit-for-bit identical output on
//! [`ComputeBackend::Serial`] and [`ComputeBackend::CpuMulti`], at any thread
//! count, on every run**, provided the caller's function is a deterministic pure
//! function of its arguments.
//!
//! The argument is the same one [`crate::math::minimise`] makes: lane `i`'s (or
//! column `j`'s) answer is a pure function of its own samples, and **no
//! arithmetic crosses lanes or columns**. A parallel *sum* would have to
//! re-associate, and floating-point addition is not associative; a set of
//! independent difference quotients has nothing to re-associate. Both backends
//! call the same `#[inline]` kernels — [`derivative_one`] and
//! [`jacobian_column`] — and only the identity of the calling thread differs.
//!
//! Verified by the `bitwise_*` tests in `differentiate/tests.rs` on 2 048
//! derivative lanes, 512 four-dimensional Jacobian lanes and one 96-dimensional
//! Jacobian, all built with points spread over seven decades so the per-lane
//! step differs. **Measured 2026-08-13 (release, `--features parallel`, 4
//! logical cores): bit-identical on every observable field of every lane, for
//! all four [`DiffScheme`] variants, at 1, 2, 4 and 8 workers.** The
//! single-point [`derivative`] and single-lane [`jacobian`] forms are separately
//! asserted bit-identical to their one-element batches.
//!
//! The `#[ignore]`d `differentiate_thread_scaling_benchmark` re-asserts the same
//! identity while timing it, on 65 536 lanes with
//! [`DiffScheme::Central4th`] (4 evaluations per lane), best of 7, two runs:
//!
//! | Worker threads | Time (A) | Speed-up (A) | (B) | Bitwise vs serial |
//! |---|---|---|---|---|
//! | *serial reference* | 5902.59 us | 1.00x | 1.00x | — |
//! | 1 | 6008.47 us | 0.98x | 0.98x | identical |
//! | 2 | 3181.82 us | 1.86x | 1.85x | identical |
//! | 4 | 1582.50 us | 3.73x | 3.70x | identical |
//! | 8 | 1577.90 us | 3.74x | 3.83x | identical |
//!
//! The "identical" column is asserted by the benchmark, not merely printed.
//! Scaling is close to linear to 4 workers and flat beyond, which is what four
//! logical cores should do. **The machine was not idle** (see
//! [`DERIVATIVE_BATCH_MIN_POINTS`] for the load); one machine, one batch, two
//! runs, nothing measured on Android hardware or a many-core server.
//!
//! The one way a caller can break this is to supply a function that is not pure
//! — one that reads a random number generator, accumulates into shared
//! interior-mutable state, or depends on the calling thread. The `Sync` bound
//! permits it; this contract forbids it.
//!
//! # Failure is reported, never swallowed
//!
//! - Every lane and every Jacobian carries a [`DiffStatus`].
//! - [`DerivativeSolution::derivative`] and [`JacobianSolution::matrix`] return
//!   `Option`, `Some` **only** on success. The diagnostic values are behind the
//!   deliberately-named [`DerivativeSolution::raw_value`] and
//!   [`JacobianSolution::raw_matrix`].
//! - [`DerivativeBatch::values`] and [`JacobianBatch::matrices`] are
//!   all-or-nothing: they return [`DiffBatchFailure`] naming the failure count
//!   and the first failing lane, rather than a `Vec` with a `NaN` in it.
//! - A failed Jacobian column is `NaN`, never `0.0`, and
//!   [`JacobianSolution::first_bad_column`] names the offending **variable**.
//!
//! **But read the limits of that guarantee** — the "What is detected, and what
//! cannot be" section on [`derivative`] lists three classes of bad input that no
//! finite-difference kernel can detect, chief among them a singularity that a
//! symmetric stencil steps over.
//!
//! # What is deliberately NOT here: dual-number autodiff
//!
//! Bead `op-yvj.4.6` offers forward-mode dual numbers as an optional exact
//! alternative, *"only if it stays simple"*. It is not implemented, on purpose.
//! Making it useful means every function a caller wants differentiated must be
//! generic over the scalar type, which would push a type parameter through
//! `OdeSystem`, through the thermophysics kernels, and into every caller's own
//! code. That is precisely the rise in reader context load the crate-level
//! "Human interface layer" rule forbids, and the bead itself ranks that rule
//! above the convenience. A caller who wants exact derivatives should hand-code
//! them; that is what [`crate::ode::OdeSystem::jacobian`] is for.
//!
//! # Units
//!
//! Everything here is dimensionless `f64`, and that is a deliberate decision
//! rather than `uom` being stripped.
//!
//! **A derivative changes dimension.** `d(enthalpy)/d(temperature)` is a heat
//! capacity; `d(pressure)/d(volume)` is none of the three. A generic
//! differentiator therefore has no single `uom` type it could return — the
//! output type is a *function* of two input types, which Rust can only express
//! through a trait with an associated output type, i.e. exactly the generic
//! machinery the "Human interface layer" rule forbids adding for its own sake.
//! The bead anticipates this and directs that, where the generic form cannot be
//! typed cleanly, a small number of **concrete typed wrappers** is preferred
//! over one generic nobody can read.
//!
//! So: `uom` typing is applied **at the boundary, by the caller** — convert in,
//! convert out — exactly as [`crate::math::minimise`] and
//! [`crate::math::parallel`] do. The one place a dimension does appear in this
//! module's own API is [`DiffSettings::min_scale`], which carries the units of
//! the variable being perturbed; its documentation says so, because a caller
//! differentiating with respect to a pressure in pascals near zero must not
//! leave it at `1.0`.
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
//! use outram_foam_basic_lib::math::differentiate::{DiffSettings, NumericalJacobian};
//! use outram_foam_basic_lib::ode::{OdeSystem, Rosenbrock23};
//!
//! // A stiff system with NO hand-coded Jacobian. Without the wrapper the
//! // default `OdeSystem::jacobian` would panic inside Rosenbrock23.
//! struct StiffPair;
//! impl OdeSystem for StiffPair {
//!     fn n_eqns(&self) -> usize { 2 }
//!     fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
//!         dydx.clear();
//!         dydx.push(-1000.0 * y[0] + y[1]);
//!         dydx.push(y[0] - y[1]);
//!     }
//! }
//!
//! let system = NumericalJacobian::new(StiffPair, DiffSettings::central());
//! let mut solver = Rosenbrock23::new(2, 1e-10, 1e-10);
//! let mut y = vec![1.0_f64, 1.0];
//! let mut dx = 1e-6;
//! solver.integrate(&system, 0.0, 1.0, &mut y, &mut dx).expect("integrates");
//!
//! // The fast mode has decayed; the slow mode (eigenvalue about -0.999) remains.
//! assert!(y[1].abs() < 1.0 && y[1] > 0.0, "y1 = {}", y[1]);
//! // ALWAYS check this -- the solver does not report a NaN Jacobian itself.
//! assert_eq!(system.non_finite_jacobians(), 0);
//! ```

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::compute::ComputeBackend;
use crate::matrix::SquareMatrix;
use crate::ode::OdeSystem;

#[cfg(test)]
mod tests;

// ── Constants ────────────────────────────────────────────────────────────────

/// `f64::EPSILON.cbrt()` = `6.0554544523933395e-6` — the relative step that
/// balances truncation against round-off for a **central** difference.
///
/// A central difference has truncation error proportional to `h^2` (the
/// coefficient is the third derivative over six) and round-off error
/// proportional to `eps/h`; minimising their sum over `h` gives
/// `h ~ eps^(1/3)`. The resulting accuracy is `~ eps^(2/3) = 3.67e-11`, **not**
/// `eps` — see the module-level "Achievable accuracy" table for the measured
/// value.
///
/// This is the same constant `outram-park-fork-offbeat`'s
/// `rheology::aster::integration::perturbed_default()` returns, for exactly the
/// same reason; see the module-level "Provenance" section.
///
/// # Units
///
/// Dimensionless — it multiplies a length scale in `x`.
pub const CBRT_EPSILON: f64 = 6.055_454_452_393_339_5e-6;

/// `f64::EPSILON.powf(0.2)` = `7.40095979741405e-4` — the relative step that
/// balances truncation against round-off for the **fourth-order** Richardson
/// scheme [`DiffScheme::Central4th`].
///
/// Truncation error proportional to `h^4`, round-off proportional to `eps/h`,
/// so the balance is at `h ~ eps^(1/5)` and the accuracy is
/// `~ eps^(4/5) = 3.00e-13`.
///
/// # Units
///
/// Dimensionless.
pub const FIFTH_ROOT_EPSILON: f64 = 7.400_959_797_414_05e-4;

/// Point count below which a [`ComputeBackend::CpuMulti`] request runs
/// [`derivative_batch`] on the calling thread instead.
///
/// # Measured crossover
///
/// *Methodology.* Measured 2026-08-13 on this workspace's development machine,
/// `std::thread::available_parallelism()` = **4**, release build,
/// `--features parallel`, `rayon`'s global pool. **The machine was NOT idle:**
/// 1-minute load average was 2.3-3.6 on 4 cores throughout, with a
/// `bn daemon run` process holding a steady ~37% of one core. Batches of `n`
/// points spread over seven decades of magnitude, [`DiffSettings::central`],
/// best of 7 samples per point, wall clock for one whole batch. Produced by the
/// `#[ignore]`d `differentiate_crossover_benchmark` test and transcribed from
/// its printed output. `cheap` is a two-flop parabola; `costly` adds an
/// `ln`/`exp`/`sqrt`/`tanh` chain, standing in for a property evaluation. Two
/// independent runs are carried side by side rather than averaged, because the
/// parallel column is far noisier than the serial one.
///
/// | Points | cheap serial | cheap speed-up (A) | (B) | costly serial | costly speed-up (A) | (B) |
/// |---|---|---|---|---|---|---|
/// | 16 | 0.18 us | 0.03x | 0.01x | 1.15 us | 0.14x | 0.04x |
/// | 32 | 0.29 us | 0.03x | 0.01x | 2.11 us | 0.20x | 0.07x |
/// | 64 | 0.51 us | 0.06x | 0.02x | 4.09 us | 0.39x | 0.13x |
/// | 128 | 0.97 us | 0.10x | 0.04x | 8.07 us | 0.47x | 0.23x |
/// | 256 | 1.87 us | 0.17x | 0.06x | 16.05 us | 0.71x | 0.40x |
/// | 512 | 3.70 us | 0.27x | 0.11x | 31.85 us | 0.74x | 0.67x |
/// | 1 024 | 7.33 us | 0.38x | 0.19x | 63.41 us | 0.85x | 1.12x |
/// | 4 096 | 28.86 us | 0.56x | 0.52x | 253.71 us | 1.52x | 2.35x |
/// | 16 384 | 115.53 us | 0.99x | 1.34x | 1025.92 us | 1.82x | 2.50x |
/// | 65 536 | 484.26 us | **1.34x** | **1.35x** | 4224.20 us | 1.90x | 2.74x |
///
/// *Result.* **65 536** is the smallest size at which the cheap objective won in
/// *both* runs, and it is the value this constant takes. That is 16x above the
/// crate-wide [`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`] placeholder and 256x
/// above [`crate::math::minimise::MINIMISE_BATCH_MIN_PROBLEMS`], and the reason
/// is structural rather than accidental: a scalar finite difference is **two to
/// four evaluations of the caller's function and one division**, so with a cheap
/// function the kernel is memory-bandwidth bound in exactly the way
/// [`crate::fields::parallel`] is — and it lands within a factor of two of that
/// module's independently measured 131 072. A batched root find, by contrast,
/// runs *tens* of iterations per lane and crosses over at 256.
///
/// **The caller's function cost moves this by more than an order of magnitude.**
/// The costly objective first wins at 1 024-4 096, sixteen to sixty-four times
/// lower. A caller who knows its function is expensive should name
/// [`ComputeBackend::CpuMulti`] explicitly rather than trust this number.
///
/// # Limitations
///
/// One machine, four logical cores, under load, one objective family. Not
/// measured on Android/Termux hardware and not on a many-core server. The
/// absolute timings should be read as ratios only.
///
/// # Units
///
/// A count of independent points, dimensionless.
pub const DERIVATIVE_BATCH_MIN_POINTS: usize = 65_536;

/// Lane count below which a [`ComputeBackend::CpuMulti`] request runs
/// [`jacobian_batch`] on the calling thread instead.
///
/// # Measured crossover
///
/// *Methodology.* Same machine, date, build and load as
/// [`DERIVATIVE_BATCH_MIN_POINTS`] — 4 logical cores, load average 2.3-3.6,
/// **not idle**. `n = 4` Jacobians, [`DiffSettings::central`] so each lane costs
/// 8 evaluations of a 4-component function, best of 7, two independent runs.
/// Produced by the same `#[ignore]`d `differentiate_crossover_benchmark` test.
///
/// | Lanes | cheap serial | cheap speed-up (A) | (B) | costly speed-up (A) | (B) |
/// |---|---|---|---|---|---|
/// | 4 | 1.56 us | 0.19x | 0.16x | 0.21x | 0.21x |
/// | 8 | 3.05 us | 0.10x | 0.12x | 0.63x | 0.35x |
/// | 16 | 6.05 us | 0.19x | 0.21x | 0.60x | 0.65x |
/// | 32 | 12.06 us | 0.83x | 0.33x | 1.52x | 1.03x |
/// | 64 | 23.64 us | 0.57x | 0.70x | 1.18x | 2.18x |
/// | 128 | 48.36 us | 0.84x | 1.34x | 1.86x | 2.46x |
/// | 256 | 95.16 us | **1.09x** | **1.11x** | 1.88x | 2.61x |
/// | 1 024 | 373.05 us | 1.63x | 2.18x | 1.86x | 1.50x |
/// | 4 096 | 1590.74 us | 1.75x | 1.90x | 1.85x | 3.10x |
/// | 16 384 | 6702.36 us | 1.83x | 2.98x | 2.06x | 2.88x |
///
/// *Result.* **256** — the smallest lane count that won in both runs. It lands
/// on exactly the same value as
/// [`crate::math::minimise::MINIMISE_BATCH_MIN_PROBLEMS`] and
/// [`crate::math::parallel::ROOT_BATCH_MIN_PROBLEMS`], and 256x *below* this
/// module's own [`DERIVATIVE_BATCH_MIN_POINTS`]. The two numbers in one module
/// disagreeing by 256x is the clearest evidence yet for the point
/// [`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`] makes: the crossover tracks
/// **work per lane**, not the algorithm. A Jacobian lane here does 8 function
/// evaluations, four vector allocations and a matrix assembly; a scalar
/// derivative lane does 2 evaluations and a division.
///
/// # A performance trap found by this measurement, recorded so it is not reintroduced
///
/// The first version of the central-difference column copied the point **twice**
/// per column (`x.to_vec()` for the `+h` probe and again for the `-h` probe).
/// With that version the parallel path **never won at any size measured**,
/// topping out at 0.96x on 16 384 lanes — allocation traffic, not arithmetic,
/// was the whole cost. Reusing a single probe buffer made the serial path ~1.4x
/// faster *and* restored parallel scaling to 1.8-3.0x. If a future change
/// reintroduces a per-column allocation, this crossover is the measurement that
/// will notice.
///
/// # Limitations
///
/// As for [`DERIVATIVE_BATCH_MIN_POINTS`], plus: measured at `n = 4` only. A
/// larger `n` raises the per-lane cost and should lower this crossover, but that
/// has not been measured.
///
/// # Units
///
/// A count of independent Jacobian problems, dimensionless.
pub const JACOBIAN_BATCH_MIN_PROBLEMS: usize = 256;

/// Dimension below which [`jacobian`] computes one Jacobian's columns on the
/// calling thread rather than spreading them across `rayon`.
///
/// This is the **other** parallel axis, and it is the one bead `op-yvj.4.6`
/// names: an `n`-dimensional Jacobian's `n + 1` (or `2n`, or `4n`) evaluations
/// are all independent.
///
/// # Measured crossover
///
/// *Methodology.* Same machine, date, build and load as
/// [`DERIVATIVE_BATCH_MIN_POINTS`] — 4 logical cores, load average 2.3-2.9,
/// **not idle**. One Jacobian of dimension `n`, [`DiffSettings::central`] so
/// `2n` evaluations, of a residual whose every component sums over all `n`
/// inputs — so one evaluation is `O(n^2)` and the whole Jacobian is `O(n^3)`,
/// the shape a genuinely coupled residual has. Best of 7, two independent runs.
/// Produced by the `#[ignore]`d `jacobian_column_crossover_benchmark` test.
///
/// | Dimension | `f` evals | serial | speed-up (A) | (B) |
/// |---|---|---|---|---|
/// | 4 | 8 | 0.72 us | 0.09x | 0.09x |
/// | 8 | 16 | 3.03 us | 0.35x | 0.16x |
/// | 16 | 32 | 15.93 us | 0.41x | 0.46x |
/// | 32 | 64 | 103.63 us | **1.49x** | **1.53x** |
/// | 64 | 128 | 767.15 us | 2.67x | 2.58x |
/// | 128 | 256 | 6117.53 us | 1.98x | 2.55x |
/// | 256 | 512 | 48189.43 us | 4.04x | 2.84x |
/// | 512 | 1024 | 389479.48 us | 3.97x | 3.55x |
///
/// *Result.* **32** — the lowest dimension that won in both runs, and the
/// smallest crossover anywhere in this crate. That is not surprising once the
/// cost is written down: at `n = 32` a single Jacobian is already 64 evaluations
/// of an `O(n^2)` residual, which is far more work per dispatch than 32 lanes of
/// anything else in the crate.
///
/// **This crossover is even more caller-dependent than the others**, because it
/// scales with the residual's own cost in `n`. A residual that is `O(1)` per
/// component rather than `O(n)` will cross over much later. 32 is set for the
/// coupled case; a caller with a cheap decoupled residual should pass
/// [`ComputeBackend::Serial`] explicitly.
///
/// # Why an ODE Jacobian does not use this
///
/// [`ode_system_jacobian`] and [`NumericalJacobian`] always run their columns
/// serially, whatever the dimension. An ODE system's `n` is its equation count —
/// typically single or double digits, well under this threshold — and the
/// parallel axis that matters for ODE work is the *ensemble lane*, which
/// [`crate::ode::parallel::integrate_ensemble`] already provides. Nesting a
/// `rayon` map inside that one would only contend for the same pool.
///
/// # Limitations
///
/// As for [`DERIVATIVE_BATCH_MIN_POINTS`], plus: one residual shape (`O(n)` per
/// component). The 128 row losing ground to both its neighbours in run A
/// (1.98x against 2.67x and 4.04x) is measurement noise on a loaded machine, not
/// a real effect — do not read this table as a scaling study.
///
/// # Units
///
/// A count of Jacobian columns, i.e. the length of the point. Dimensionless.
pub const JACOBIAN_COLUMN_MIN_DIMENSION: usize = 32;

// ── Backend dispatch ─────────────────────────────────────────────────────────

/// Resolve a requested backend to the one this module will actually run.
///
/// Three reductions, in order: [`ComputeBackend::resolve`] degrades anything
/// whose feature is off; `Gpu` degrades again because this module has no GPU
/// kernel yet; and `CpuMulti` degrades to `Serial` below `min_work_items`. The
/// result is only ever `Serial` or `CpuMulti`, and none of the degradations can
/// change a returned value.
///
/// Identical in shape to `minimise::effective_backend` and
/// `parallel::effective_backend`; kept private and duplicated rather than
/// hoisted because all three are four lines and hoisting would put a
/// dispatch-policy helper in a module whose docs say it holds no kernels.
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

/// The [`ComputeBackend`] [`derivative_batch`] would actually use for `n`
/// points if asked for `requested` — without running anything.
///
/// Applies exactly the same reduction the kernel does (feature availability, no
/// GPU kernel here, and the [`DERIVATIVE_BATCH_MIN_POINTS`] size floor), so what
/// it reports is what would run.
///
/// # Arguments
///
/// - `requested` — the backend a caller would pass to [`derivative_batch`].
/// - `n` — number of independent points in the batch, dimensionless.
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
/// use outram_foam_basic_lib::math::differentiate::{
///     derivative_backend_for, DERIVATIVE_BATCH_MIN_POINTS,
/// };
///
/// assert_eq!(
///     derivative_backend_for(ComputeBackend::CpuMulti, 8),
///     ComputeBackend::Serial
/// );
/// assert!(derivative_backend_for(ComputeBackend::CpuMulti, DERIVATIVE_BATCH_MIN_POINTS)
///     .is_available());
/// ```
#[must_use]
pub fn derivative_backend_for(requested: ComputeBackend, n: usize) -> ComputeBackend {
    effective_backend(requested, n, DERIVATIVE_BATCH_MIN_POINTS)
}

/// The [`ComputeBackend`] [`jacobian_batch`] would actually use for `n`
/// independent Jacobian problems — without running anything.
///
/// # Arguments
///
/// - `requested` — the backend a caller would pass to [`jacobian_batch`].
/// - `n` — number of independent Jacobian problems (lanes), dimensionless.
///
/// # Returns
///
/// Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`].
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::differentiate::jacobian_batch_backend_for;
///
/// assert_eq!(
///     jacobian_batch_backend_for(ComputeBackend::CpuMulti, 2),
///     ComputeBackend::Serial
/// );
/// ```
#[must_use]
pub fn jacobian_batch_backend_for(requested: ComputeBackend, n: usize) -> ComputeBackend {
    effective_backend(requested, n, JACOBIAN_BATCH_MIN_PROBLEMS)
}

/// The [`ComputeBackend`] [`jacobian`] would actually use to spread the columns
/// of **one** `dimension`-dimensional Jacobian — without running anything.
///
/// This is the *other* axis of parallelism in this module: [`jacobian_batch`]
/// spreads independent problems across threads, while [`jacobian`] spreads the
/// `n` independent column evaluations of a single problem.
///
/// # Arguments
///
/// - `requested` — the backend a caller would pass to [`jacobian`].
/// - `dimension` — the length of the point `x`, i.e. the number of Jacobian
///   columns. Dimensionless.
///
/// # Returns
///
/// Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`].
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::differentiate::jacobian_column_backend_for;
///
/// // A 3-equation ODE Jacobian is never worth threading.
/// assert_eq!(
///     jacobian_column_backend_for(ComputeBackend::CpuMulti, 3),
///     ComputeBackend::Serial
/// );
/// ```
#[must_use]
pub fn jacobian_column_backend_for(requested: ComputeBackend, dimension: usize) -> ComputeBackend {
    effective_backend(requested, dimension, JACOBIAN_COLUMN_MIN_DIMENSION)
}

// ── Scheme, settings ─────────────────────────────────────────────────────────

/// Which finite-difference stencil to use.
///
/// The choice is a **cost against accuracy** trade, and both halves are
/// measured — see the module-level "Achievable accuracy" table for the observed
/// error floors and "Cost" for the evaluation counts.
///
/// # Units
///
/// Dimensionless — a mode selector, not a quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DiffScheme {
    /// `(f(x + h) - f(x)) / h`. Truncation error `O(h)`.
    ///
    /// The cheapest scheme for a Jacobian: the base evaluation `f(x)` is shared
    /// by every column, so an `n`-dimensional Jacobian costs `n + 1`
    /// evaluations rather than `2n`.
    Forward,
    /// `(f(x) - f(x - h)) / h`. Truncation error `O(h)`.
    ///
    /// Same cost and accuracy as [`Forward`](Self::Forward); it exists for
    /// callers whose function is undefined or unphysical just *above* `x` — a
    /// saturation pressure at the phase boundary, a volume fraction at 1.
    Backward,
    /// `(f(x + h) - f(x - h)) / (2h)`. Truncation error `O(h^2)`. **The
    /// default.**
    ///
    /// This is the scheme both existing workspace implementations use — see the
    /// module-level "Provenance" section.
    Central,
    /// Richardson extrapolation of two central differences, `(4*D(h/2) -
    /// D(h)) / 3`. Truncation error `O(h^4)`.
    ///
    /// The most accurate scheme here and the most expensive: 4 evaluations per
    /// derivative and `4n` per Jacobian, because the `h` and `h/2` stencils
    /// share no points.
    #[default]
    Central4th,
}

impl DiffScheme {
    /// The relative step size that balances truncation against round-off for
    /// this scheme in `f64`.
    ///
    /// Truncation error goes as `h^p` and round-off as `eps/h`, so the balance
    /// is at `h ~ eps^(1/(p+1))`:
    ///
    /// | Scheme | Order `p` | Optimal relative step |
    /// |---|---|---|
    /// | [`Forward`](Self::Forward), [`Backward`](Self::Backward) | 1 | [`crate::math::minimise::SQRT_EPSILON`] = `1.4901161193847656e-8` |
    /// | [`Central`](Self::Central) | 2 | [`CBRT_EPSILON`] = `6.0554544523933395e-6` |
    /// | [`Central4th`](Self::Central4th) | 4 | [`FIFTH_ROOT_EPSILON`] = `7.40095979741405e-4` |
    ///
    /// # Units
    ///
    /// Dimensionless — it multiplies a length scale in `x` to give a step in
    /// `x`.
    #[must_use]
    pub fn default_relative_step(self) -> f64 {
        match self {
            Self::Forward | Self::Backward => crate::math::minimise::SQRT_EPSILON,
            Self::Central => CBRT_EPSILON,
            Self::Central4th => FIFTH_ROOT_EPSILON,
        }
    }

    /// How many evaluations of the function one **scalar** derivative costs.
    ///
    /// # Units
    ///
    /// A count, dimensionless.
    #[must_use]
    pub fn evaluations_per_derivative(self) -> usize {
        match self {
            Self::Forward | Self::Backward | Self::Central => 2,
            Self::Central4th => 4,
        }
    }

    /// How many evaluations of the vector function an `n`-dimensional Jacobian
    /// costs.
    ///
    /// [`Forward`](Self::Forward) and [`Backward`](Self::Backward) get `n + 1`
    /// because the unperturbed evaluation is shared across all `n` columns;
    /// [`Central`](Self::Central) gets `2n` and [`Central4th`](Self::Central4th)
    /// `4n` because their stencils are symmetric about `x` and so share nothing.
    ///
    /// # Arguments
    ///
    /// - `n` — the dimension of the point, dimensionless.
    ///
    /// # Units
    ///
    /// A count, dimensionless.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::math::differentiate::DiffScheme;
    ///
    /// assert_eq!(DiffScheme::Forward.evaluations_per_jacobian(10), 11);
    /// assert_eq!(DiffScheme::Central.evaluations_per_jacobian(10), 20);
    /// assert_eq!(DiffScheme::Central4th.evaluations_per_jacobian(10), 40);
    /// ```
    #[must_use]
    pub fn evaluations_per_jacobian(self, n: usize) -> usize {
        match self {
            Self::Forward | Self::Backward => n + 1,
            Self::Central => 2 * n,
            Self::Central4th => 4 * n,
        }
    }

    /// A short human-readable label, for benchmark tables and log lines.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Backward => "backward",
            Self::Central => "central",
            Self::Central4th => "central-4th",
        }
    }
}

/// Step-size policy for every entry point in this module.
///
/// # The step-size rule
///
/// ```text
/// h = relative_step * max(|x|, min_scale)
/// ```
///
/// The step is **relative to the magnitude of the variable being perturbed**,
/// because a step that is right for `x ~ 1` is far too small for a pressure in
/// pascals and far too large for a mole fraction. `min_scale` is the floor that
/// keeps the rule usable at `x = 0` — see [`Self::step_for`].
///
/// # Units
///
/// `relative_step` is dimensionless. `min_scale` carries the **same units as
/// the variable being differentiated with respect to**, because it is a
/// fallback magnitude for `x`, and its default of `1.0` therefore means "one of
/// whatever unit `x` is in". A caller differentiating with respect to a
/// pressure in pascals near zero wants `min_scale` set to a pascal-scale
/// number, not `1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiffSettings {
    /// Which stencil to use. See [`DiffScheme`].
    pub scheme: DiffScheme,
    /// The relative step, dimensionless. Defaults to
    /// [`DiffScheme::default_relative_step`] for the chosen scheme.
    pub relative_step: f64,
    /// Floor on `|x|` in the step rule, so `x = 0` still gets a usable step.
    /// Same units as `x`. Default `1.0`.
    pub min_scale: f64,
}

impl Default for DiffSettings {
    /// [`DiffScheme::Central4th`] with its optimal relative step and
    /// `min_scale = 1.0`.
    ///
    /// The default is the *most accurate* scheme rather than the cheapest,
    /// because a caller who has not thought about step size is far more likely
    /// to be surprised by a wrong derivative than by four function evaluations.
    fn default() -> Self {
        Self::with_scheme(DiffScheme::Central4th)
    }
}

impl DiffSettings {
    /// Settings for `scheme`, with that scheme's optimal relative step and
    /// `min_scale = 1.0`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::math::differentiate::{DiffScheme, DiffSettings};
    ///
    /// let s = DiffSettings::with_scheme(DiffScheme::Central);
    /// assert_eq!(s.relative_step, DiffScheme::Central.default_relative_step());
    /// assert_eq!(s.min_scale, 1.0);
    /// ```
    #[must_use]
    pub fn with_scheme(scheme: DiffScheme) -> Self {
        Self {
            scheme,
            relative_step: scheme.default_relative_step(),
            min_scale: 1.0,
        }
    }

    /// [`DiffScheme::Forward`] with its optimal relative step — the `n + 1`
    /// evaluation Jacobian.
    #[must_use]
    pub fn forward() -> Self {
        Self::with_scheme(DiffScheme::Forward)
    }

    /// [`DiffScheme::Backward`] with its optimal relative step.
    #[must_use]
    pub fn backward() -> Self {
        Self::with_scheme(DiffScheme::Backward)
    }

    /// [`DiffScheme::Central`] with its optimal relative step — the `2n`
    /// evaluation Jacobian, and the scheme both existing workspace
    /// implementations use.
    #[must_use]
    pub fn central() -> Self {
        Self::with_scheme(DiffScheme::Central)
    }

    /// [`DiffScheme::Central4th`] with its optimal relative step — the `4n`
    /// evaluation Jacobian. Same as [`Self::default`].
    #[must_use]
    pub fn central_4th() -> Self {
        Self::with_scheme(DiffScheme::Central4th)
    }

    /// The step this policy uses to perturb a variable currently at `x`.
    ///
    /// ```text
    /// h = relative_step * max(|x|, min_scale)
    /// ```
    ///
    /// # What happens at `x = 0`
    ///
    /// A purely relative step `relative_step * |x|` is **exactly zero** at
    /// `x = 0`, which would divide by zero and hand back `NaN` or `inf`. The
    /// `max(|x|, min_scale)` floor is what prevents that: at `x = 0` the step
    /// becomes `relative_step * min_scale`, i.e. an *absolute* step of
    /// `relative_step` in the default `min_scale = 1.0` case.
    ///
    /// This is the convention already settled elsewhere in this workspace —
    /// `outram-park-fork-offbeat`'s `newton_perturbed` uses
    /// `perturbation * x.abs().max(1.0)` for exactly this reason. It is
    /// **not** the convention `outram-park-fork-dwsim-libs`'
    /// `finite_difference_jacobian` uses; see the module-level "Provenance"
    /// section for that divergence and why.
    ///
    /// The same floor also rescues the near-zero case, which is the one that
    /// actually bites: at `x = 1e-300` a relative step is `1e-308`-ish, so
    /// `x + h` rounds straight back to `x` and the difference is identically
    /// zero. That is reported as [`DiffStatus::DegenerateStep`], not as a
    /// derivative of zero.
    ///
    /// # Returns
    ///
    /// The step, in the same units as `x`. Non-finite or non-positive results
    /// are possible if the caller supplies a nonsensical `relative_step`, and
    /// are caught by the kernels rather than by this function.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::math::differentiate::DiffSettings;
    ///
    /// let s = DiffSettings::central();
    /// // Relative where |x| is large ...
    /// assert_eq!(s.step_for(1000.0), s.relative_step * 1000.0);
    /// // ... absolute where it is not, so x = 0 still works.
    /// assert_eq!(s.step_for(0.0), s.relative_step);
    /// assert_eq!(s.step_for(-1000.0), s.relative_step * 1000.0);
    /// ```
    #[must_use]
    pub fn step_for(&self, x: f64) -> f64 {
        self.relative_step * x.abs().max(self.min_scale)
    }
}

// ── Status ───────────────────────────────────────────────────────────────────

/// Why a derivative or Jacobian entry is, or is not, trustworthy.
///
/// # Units
///
/// Dimensionless — a status code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffStatus {
    /// The difference quotient was formed from finite evaluations with a
    /// non-degenerate step. The value is usable.
    Ok,
    /// The point `x` itself was not finite, so no step could be taken.
    InvalidPoint,
    /// At least one function evaluation returned a non-finite value, or the
    /// difference quotient itself came out non-finite (overflow in the
    /// subtraction, for instance).
    NotFinite,
    /// The step collapsed: `relative_step` was zero, negative or non-finite, or
    /// `x + h` rounded back to `x` so the realised step was exactly zero. The
    /// quotient would have been a division by zero.
    DegenerateStep,
    /// The vector function returned a different number of components than the
    /// point has, so the Jacobian is not square and cannot be assembled.
    ///
    /// Only reachable from [`jacobian`] and its batched form; the square
    /// restriction is documented on [`jacobian`].
    DimensionMismatch,
}

impl DiffStatus {
    /// Whether the value this status accompanies may be used.
    #[must_use]
    pub fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    /// A short human-readable label, for log lines and benchmark tables.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::InvalidPoint => "invalid-point",
            Self::NotFinite => "not-finite",
            Self::DegenerateStep => "degenerate-step",
            Self::DimensionMismatch => "dimension-mismatch",
        }
    }
}

// ── Scalar derivative ────────────────────────────────────────────────────────

/// One lane's scalar derivative, with the diagnostics needed to judge it.
///
/// # Units
///
/// [`value`](Self::value) carries the units of `f` divided by the units of `x`
/// — a derivative changes dimension, which is exactly why this module does not
/// try to `uom`-type the generic form. See the module-level "Units" section.
/// [`realised_step`](Self::realised_step) carries the units of `x`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DerivativeSolution {
    value: f64,
    realised_step: f64,
    status: DiffStatus,
}

impl DerivativeSolution {
    /// The derivative, **only if** this lane succeeded.
    ///
    /// Returns `None` for every non-[`DiffStatus::Ok`] status, so a caller
    /// cannot accidentally consume a `NaN` as a derivative. The diagnostic
    /// number is behind the deliberately-named [`Self::raw_value`].
    #[must_use]
    pub fn derivative(&self) -> Option<f64> {
        if self.status.is_ok() {
            Some(self.value)
        } else {
            None
        }
    }

    /// The difference quotient as computed, whatever the status — a diagnostic,
    /// **not** an answer. Frequently `NaN`.
    #[must_use]
    pub fn raw_value(&self) -> f64 {
        self.value
    }

    /// The step actually taken, after the `x + h` rounding correction described
    /// on [`derivative`]. Units of `x`.
    ///
    /// This is the denominator that was really divided by, not the `h` that
    /// [`DiffSettings::step_for`] asked for, and comparing the two is the
    /// cheapest way to see step-size trouble.
    #[must_use]
    pub fn realised_step(&self) -> f64 {
        self.realised_step
    }

    /// Why this lane succeeded or failed.
    #[must_use]
    pub fn status(&self) -> DiffStatus {
        self.status
    }

    /// Whether this lane produced a usable derivative.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.status.is_ok()
    }

    fn ok(value: f64, realised_step: f64) -> Self {
        Self {
            value,
            realised_step,
            status: DiffStatus::Ok,
        }
    }

    fn failed(status: DiffStatus, realised_step: f64) -> Self {
        Self {
            value: f64::NAN,
            realised_step,
            status,
        }
    }
}

/// One or more lanes of a [`DerivativeBatch`] or [`JacobianBatch`] failed.
///
/// Returned by the all-or-nothing accessors [`DerivativeBatch::values`] and
/// [`JacobianBatch::matrices`]. It names both the scale of the problem (how
/// many of how many) and a specific lane to look at, because "3 of 10 000 lanes
/// failed" is only actionable once you know *which* lane.
///
/// # Units
///
/// All counts and indices are dimensionless.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "{failure_count} of {total} differentiation lanes failed; \
     first failure at lane {first_index} with status {first_status:?}"
)]
pub struct DiffBatchFailure {
    /// Number of lanes in the batch.
    pub total: usize,
    /// Number of lanes that failed.
    pub failure_count: usize,
    /// Index of the first failing lane.
    pub first_index: usize,
    /// Why that lane failed.
    pub first_status: DiffStatus,
}

/// The result of [`derivative_batch`] — one [`DerivativeSolution`] per point,
/// in point order.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivativeBatch {
    solutions: Vec<DerivativeSolution>,
}

impl DerivativeBatch {
    /// Every lane's solution, in the order the points were supplied.
    #[must_use]
    pub fn solutions(&self) -> &[DerivativeSolution] {
        &self.solutions
    }

    /// Consume the batch, yielding the per-lane solutions.
    #[must_use]
    pub fn into_solutions(self) -> Vec<DerivativeSolution> {
        self.solutions
    }

    /// Number of lanes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.solutions.len()
    }

    /// Whether the batch has no lanes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.solutions.is_empty()
    }

    /// Lane `i`'s solution, or `None` if `i` is out of range.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<DerivativeSolution> {
        self.solutions.get(i).copied()
    }

    /// Whether every lane produced a usable derivative.
    #[must_use]
    pub fn all_ok(&self) -> bool {
        self.solutions.iter().all(DerivativeSolution::is_ok)
    }

    /// How many lanes failed.
    #[must_use]
    pub fn failure_count(&self) -> usize {
        self.solutions.iter().filter(|s| !s.is_ok()).count()
    }

    /// The first failing lane and its solution, if any.
    #[must_use]
    pub fn first_failure(&self) -> Option<(usize, DerivativeSolution)> {
        self.solutions
            .iter()
            .enumerate()
            .find(|(_, s)| !s.is_ok())
            .map(|(i, s)| (i, *s))
    }

    /// Every failing lane and its solution.
    #[must_use]
    pub fn failures(&self) -> Vec<(usize, DerivativeSolution)> {
        self.solutions
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.is_ok())
            .map(|(i, s)| (i, *s))
            .collect()
    }

    /// Every lane's derivative, **all or nothing**.
    ///
    /// # Errors
    ///
    /// [`DiffBatchFailure`] naming the failure count and the first failing lane
    /// if any lane failed. A partially-`NaN` `Vec<f64>` is never returned.
    pub fn values(&self) -> Result<Vec<f64>, DiffBatchFailure> {
        self.check_all_ok()?;
        Ok(self.solutions.iter().map(|s| s.value).collect())
    }

    /// `Err` describing the first failure, if any lane failed.
    ///
    /// # Errors
    ///
    /// [`DiffBatchFailure`] as for [`Self::values`].
    pub fn check_all_ok(&self) -> Result<(), DiffBatchFailure> {
        if let Some((i, s)) = self.first_failure() {
            return Err(DiffBatchFailure {
                total: self.solutions.len(),
                failure_count: self.failure_count(),
                first_index: i,
                first_status: s.status(),
            });
        }
        Ok(())
    }
}

/// Differentiate one scalar function at one point.
///
/// The single-lane form of [`derivative_batch`], for callers with one
/// derivative to take. It runs on the calling thread — there is nothing to
/// spread — and calls the *same* per-lane kernel, so it agrees with a
/// one-element batch bit for bit.
///
/// # The realised-step correction
///
/// `x + h` is generally not representable, so the value the machine actually
/// evaluates at differs from `x + h` in the last bits and the true step is not
/// `h`. This kernel therefore evaluates at `xp = x + h` and divides by
/// `xp - x`, which **is** exact, rather than by `h`. The device is from
/// *Numerical Recipes* (Press et al., 3rd ed., section 5.7) and it removes an
/// error source that would otherwise be comparable to the round-off term the
/// step rule is trying to balance. [`DerivativeSolution::realised_step`]
/// reports the corrected denominator.
///
/// # Arguments
///
/// - `x` — the point, in the caller's own units.
/// - `settings` — scheme and step-size policy; see [`DiffSettings`].
/// - `f` — the function. Units of the return value are the caller's.
///
/// # Returns
///
/// A [`DerivativeSolution`] whose [`derivative`](DerivativeSolution::derivative)
/// is `Some` only if every evaluation was finite and the realised step was
/// non-zero. **That is the whole of the guarantee** — read the next section
/// before relying on it.
///
/// # What is detected, and what cannot be
///
/// The status is computed from exactly one predicate — every sampled value and
/// the resulting quotient are finite, and the realised step is non-zero — and
/// [`DerivativeSolution::derivative`] returns `Some` on exactly that same
/// predicate. They cannot disagree.
///
/// **Detected:** a non-finite sample ([`DiffStatus::NotFinite`]), a non-finite
/// point ([`DiffStatus::InvalidPoint`]), and a step that rounds away so
/// `x + h == x` ([`DiffStatus::DegenerateStep`]).
///
/// **Not detected, and not detectable by any finite-difference kernel:**
///
/// - **A singularity the stencil steps over.** `1/x` at `x = 0` is sampled by
///   the central stencil at `+h` and `-h`, both perfectly finite, so it returns
///   `1/h^2` with [`DiffStatus::Ok`]. The kernel never evaluates at the pole and
///   has no way to learn it is there. A one-sided scheme *does* see this
///   particular case, because it evaluates at `x` itself — but it has the
///   mirror-image blind spot on the other side.
/// - **Cancellation that leaves a finite number with no correct digits.** The
///   quotient is a perfectly ordinary `f64`; nothing about it says how many of
///   its bits survived. This is what the step-size rule exists to bound, and why
///   the module documents an *accuracy floor* rather than a guarantee.
/// - **A function that is not differentiable at `x`.** `|x|` at `0` returns `0`
///   from the central stencil, confidently.
///
/// If the function may be singular or kinked, bracket it away from the trouble
/// or check the result against a second scheme; the status field will not do it
/// for you.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::math::differentiate::{
///     derivative, DiffSettings, DiffStatus,
/// };
///
/// // d/dx sin(x) at x = 1 is cos(1).
/// let s = derivative(1.0, DiffSettings::central(), |x: f64| x.sin());
/// let d = s.derivative().expect("finite everywhere");
/// assert!((d - 1.0_f64.cos()).abs() < 1e-10, "got {d}");
///
/// // A sample that comes back non-finite IS reported: the central stencil for
/// // sqrt at x = 0 evaluates at -h, which is NaN.
/// let bad = derivative(0.0, DiffSettings::central(), |x: f64| x.sqrt());
/// assert_eq!(bad.status(), DiffStatus::NotFinite);
/// assert!(bad.derivative().is_none());
///
/// // But a pole the stencil STEPS OVER is not, and cannot be -- see
/// // "What is detected, and what cannot be" above.
/// let undetected = derivative(0.0, DiffSettings::central(), |x: f64| 1.0 / x);
/// assert_eq!(undetected.status(), DiffStatus::Ok);
/// assert!(undetected.derivative().is_some());
/// ```
#[must_use]
pub fn derivative<F>(x: f64, settings: DiffSettings, f: F) -> DerivativeSolution
where
    F: Fn(f64) -> f64,
{
    derivative_one(0, x, settings, &|_, t| f(t))
}

/// Differentiate `N` independent scalar functions at `N` points, on the chosen
/// backend.
///
/// This is the batched, GPU-shaped form: lane `i` differentiates `f(i, .)` at
/// `points[i]`, and no arithmetic crosses lanes.
///
/// # Arguments
///
/// - `points` — one abscissa per lane, in the caller's own units.
/// - `settings` — scheme and step-size policy, shared by every lane.
/// - `backend` — requested execution backend. What actually runs is
///   [`derivative_backend_for`] applied to it. **None of the degradations
///   changes the answer.**
/// - `f` — `f(i, x)` is lane `i`'s function evaluated at `x`. It **must be a
///   pure deterministic function of its arguments** — see the module-level
///   "Determinism" section. The `Sync` bound is present in both feature builds
///   so enabling `parallel` never changes a public signature.
///
/// # Returns
///
/// A [`DerivativeBatch`] with one solution per point, in point order. An empty
/// `points` slice returns an empty batch and calls `f` zero times.
///
/// # Cost
///
/// [`DiffScheme::evaluations_per_derivative`] calls to `f` per lane — 2 for the
/// three second-order-or-lower schemes, 4 for [`DiffScheme::Central4th`].
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::differentiate::{
///     derivative_batch, DiffSettings,
/// };
///
/// // Lane i differentiates x^(i+1) at x = 2; the answer is (i+1) * 2^i.
/// let points = vec![2.0_f64; 4];
/// let batch = derivative_batch(
///     &points,
///     DiffSettings::central(),
///     ComputeBackend::CpuMulti,
///     |i, x: f64| x.powi(i as i32 + 1),
/// );
///
/// let d = batch.values().expect("all lanes finite");
/// for (i, got) in d.iter().enumerate() {
///     let want = (i as f64 + 1.0) * 2.0_f64.powi(i as i32);
///     assert!((got - want).abs() < 1e-6 * want.abs().max(1.0), "lane {i}: {got} vs {want}");
/// }
/// ```
#[must_use]
pub fn derivative_batch<F>(
    points: &[f64],
    settings: DiffSettings,
    backend: ComputeBackend,
    f: F,
) -> DerivativeBatch
where
    F: Fn(usize, f64) -> f64 + Sync,
{
    derivative_batch_min(points, settings, backend, DERIVATIVE_BATCH_MIN_POINTS, f)
}

/// [`derivative_batch`] with the size floor supplied by the caller.
///
/// Exists so the crossover benchmark can measure the multi-CPU path *below*
/// [`DERIVATIVE_BATCH_MIN_POINTS`] — the only way to find where the crossover
/// actually is — and so the cross-backend bitwise tests are not vacuous on
/// small batches. Not public: production callers get the measured floor.
pub(crate) fn derivative_batch_min<F>(
    points: &[f64],
    settings: DiffSettings,
    backend: ComputeBackend,
    min_points: usize,
    f: F,
) -> DerivativeBatch
where
    F: Fn(usize, f64) -> f64 + Sync,
{
    let n = points.len();
    let solutions: Vec<DerivativeSolution> = match effective_backend(backend, n, min_points) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => points
            .par_iter()
            .enumerate()
            .map(|(i, &x)| derivative_one(i, x, settings, &f))
            .collect(),
        _ => points
            .iter()
            .enumerate()
            .map(|(i, &x)| derivative_one(i, x, settings, &f))
            .collect(),
    };
    DerivativeBatch { solutions }
}

// ── Per-lane scalar kernel — one implementation, both backends ───────────────

/// The single-lane finite difference that **both** backends run.
///
/// `#[inline]` so the serial loop and the `rayon` map compile to the same inner
/// code — part of why the two backends agree bit for bit.
#[inline]
fn derivative_one<F>(i: usize, x: f64, settings: DiffSettings, f: &F) -> DerivativeSolution
where
    F: Fn(usize, f64) -> f64,
{
    if !x.is_finite() {
        return DerivativeSolution::failed(DiffStatus::InvalidPoint, f64::NAN);
    }
    let h = settings.step_for(x);
    if !h.is_finite() || h <= 0.0 {
        return DerivativeSolution::failed(DiffStatus::DegenerateStep, h);
    }

    match settings.scheme {
        DiffScheme::Forward => one_sided(i, x, h, f),
        DiffScheme::Backward => one_sided(i, x, -h, f),
        DiffScheme::Central => central(i, x, h, f),
        DiffScheme::Central4th => {
            // Richardson extrapolation of two central differences.
            // D(h) has error c2*h^2 + c4*h^4 + ...; D(h/2) has c2*h^2/4 + ...
            // so (4*D(h/2) - D(h)) / 3 cancels the h^2 term exactly.
            let coarse = central(i, x, h, f);
            if !coarse.is_ok() {
                return coarse;
            }
            let fine = central(i, x, 0.5 * h, f);
            if !fine.is_ok() {
                return fine;
            }
            let value = (4.0 * fine.value - coarse.value) / 3.0;
            if value.is_finite() {
                DerivativeSolution::ok(value, fine.realised_step)
            } else {
                DerivativeSolution::failed(DiffStatus::NotFinite, fine.realised_step)
            }
        }
    }
}

/// Forward (`h > 0`) or backward (`h < 0`) difference at `x`.
#[inline]
fn one_sided<F>(i: usize, x: f64, h: f64, f: &F) -> DerivativeSolution
where
    F: Fn(usize, f64) -> f64,
{
    let xp = x + h;
    // `xp - x` is exact and is the step the machine really took; `h` is not.
    let dh = xp - x;
    if dh == 0.0 || !dh.is_finite() {
        return DerivativeSolution::failed(DiffStatus::DegenerateStep, dh);
    }
    let f0 = f(i, x);
    let f1 = f(i, xp);
    if !f0.is_finite() || !f1.is_finite() {
        return DerivativeSolution::failed(DiffStatus::NotFinite, dh);
    }
    let value = (f1 - f0) / dh;
    if value.is_finite() {
        DerivativeSolution::ok(value, dh)
    } else {
        DerivativeSolution::failed(DiffStatus::NotFinite, dh)
    }
}

/// Central difference at `x` with half-width `h`.
#[inline]
fn central<F>(i: usize, x: f64, h: f64, f: &F) -> DerivativeSolution
where
    F: Fn(usize, f64) -> f64,
{
    let xp = x + h;
    let xm = x - h;
    let dh = xp - xm;
    if dh == 0.0 || !dh.is_finite() {
        return DerivativeSolution::failed(DiffStatus::DegenerateStep, dh);
    }
    let fp = f(i, xp);
    let fm = f(i, xm);
    if !fp.is_finite() || !fm.is_finite() {
        return DerivativeSolution::failed(DiffStatus::NotFinite, dh);
    }
    let value = (fp - fm) / dh;
    if value.is_finite() {
        DerivativeSolution::ok(value, dh)
    } else {
        DerivativeSolution::failed(DiffStatus::NotFinite, dh)
    }
}

// ── Jacobians ────────────────────────────────────────────────────────────────

/// One lane's Jacobian, with the status needed to judge it.
///
/// # Units
///
/// Entry `(i, j)` of the matrix carries the units of `f_i` divided by the units
/// of `x_j`. See the module-level "Units" section for why the generic form is
/// not `uom`-typed.
#[derive(Debug, Clone)]
pub struct JacobianSolution {
    matrix: SquareMatrix,
    status: DiffStatus,
    first_bad_column: usize,
}

impl JacobianSolution {
    /// The Jacobian, **only if** every column of this lane succeeded.
    ///
    /// Returns `None` for every non-[`DiffStatus::Ok`] status, so a caller
    /// cannot accidentally factorise a partially-`NaN` matrix believing it to
    /// be a Jacobian. The diagnostic matrix is behind the deliberately-named
    /// [`Self::raw_matrix`].
    #[must_use]
    pub fn matrix(&self) -> Option<&SquareMatrix> {
        if self.status.is_ok() {
            Some(&self.matrix)
        } else {
            None
        }
    }

    /// Consume the solution, yielding the Jacobian only if it succeeded.
    #[must_use]
    pub fn into_matrix(self) -> Option<SquareMatrix> {
        if self.status.is_ok() {
            Some(self.matrix)
        } else {
            None
        }
    }

    /// The matrix as assembled, whatever the status — a diagnostic, **not** an
    /// answer. Failed columns are filled with `NaN`.
    #[must_use]
    pub fn raw_matrix(&self) -> &SquareMatrix {
        &self.matrix
    }

    /// Why this lane succeeded or failed.
    #[must_use]
    pub fn status(&self) -> DiffStatus {
        self.status
    }

    /// Whether this lane produced a usable Jacobian.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.status.is_ok()
    }

    /// The index of the first column that failed, or `usize::MAX` if none did.
    ///
    /// "Column `j`" means the derivative with respect to `x[j]`, so this points
    /// straight at the offending *variable* rather than at the offending
    /// equation — which is the useful direction, because a failed column
    /// usually means the step took `x[j]` somewhere the function is not
    /// defined.
    #[must_use]
    pub fn first_bad_column(&self) -> usize {
        self.first_bad_column
    }
}

/// The result of [`jacobian_batch`] — one [`JacobianSolution`] per lane.
#[derive(Debug, Clone)]
pub struct JacobianBatch {
    solutions: Vec<JacobianSolution>,
}

impl JacobianBatch {
    /// Every lane's solution, in the order the points were supplied.
    #[must_use]
    pub fn solutions(&self) -> &[JacobianSolution] {
        &self.solutions
    }

    /// Consume the batch, yielding the per-lane solutions.
    #[must_use]
    pub fn into_solutions(self) -> Vec<JacobianSolution> {
        self.solutions
    }

    /// Number of lanes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.solutions.len()
    }

    /// Whether the batch has no lanes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.solutions.is_empty()
    }

    /// Lane `i`'s solution, or `None` if `i` is out of range.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&JacobianSolution> {
        self.solutions.get(i)
    }

    /// Whether every lane produced a usable Jacobian.
    #[must_use]
    pub fn all_ok(&self) -> bool {
        self.solutions.iter().all(JacobianSolution::is_ok)
    }

    /// How many lanes failed.
    #[must_use]
    pub fn failure_count(&self) -> usize {
        self.solutions.iter().filter(|s| !s.is_ok()).count()
    }

    /// The first failing lane index and its status, if any.
    #[must_use]
    pub fn first_failure(&self) -> Option<(usize, DiffStatus)> {
        self.solutions
            .iter()
            .enumerate()
            .find(|(_, s)| !s.is_ok())
            .map(|(i, s)| (i, s.status()))
    }

    /// Every lane's Jacobian, **all or nothing**.
    ///
    /// # Errors
    ///
    /// [`DiffBatchFailure`] naming the failure count and the first failing lane
    /// if any lane failed. A `Vec` containing a partially-`NaN` matrix is never
    /// returned.
    pub fn matrices(self) -> Result<Vec<SquareMatrix>, DiffBatchFailure> {
        self.check_all_ok()?;
        Ok(self.solutions.into_iter().map(|s| s.matrix).collect())
    }

    /// `Err` describing the first failure, if any lane failed.
    ///
    /// # Errors
    ///
    /// [`DiffBatchFailure`] as for [`Self::matrices`].
    pub fn check_all_ok(&self) -> Result<(), DiffBatchFailure> {
        if let Some((i, status)) = self.first_failure() {
            return Err(DiffBatchFailure {
                total: self.solutions.len(),
                failure_count: self.failure_count(),
                first_index: i,
                first_status: status,
            });
        }
        Ok(())
    }
}

/// Assemble the Jacobian `J[i][j] = d f_i / d x_j` of one `n`-dimensional
/// vector function at one point, by finite differences.
///
/// The direct feeder for multi-dimensional Newton and — through
/// [`NumericalJacobian`] — for [`crate::ode::Rosenbrock23`].
///
/// # Square only
///
/// `f` must return exactly `x.len()` components. A rectangular Jacobian is
/// rejected with [`DiffStatus::DimensionMismatch`] rather than silently padded.
/// This restriction is deliberate and matches the prior art: the consumer
/// (`n` ODE equations in `n` states) is square, the crate's [`SquareMatrix`] is
/// square, and `outram-park-fork-dwsim-libs`' `finite_difference_jacobian`
/// rejects the non-square case too.
///
/// # Arguments
///
/// - `x` — the point, one component per variable, in the caller's own units.
/// - `settings` — scheme and step-size policy; see [`DiffSettings`]. The step
///   is computed per-column from that column's own `x[j]`, so variables of
///   wildly different magnitude each get an appropriate step.
/// - `backend` — requested backend for spreading the **columns** of this one
///   Jacobian. What actually runs is [`jacobian_column_backend_for`] applied to
///   it; a small `n` runs serially. **None of the degradations changes the
///   answer.**
/// - `f` — `f(0, x, out)` must fill `out` with the `n` function components at
///   `x`. The lane index is always `0` here; it is in the signature so the same
///   closure works with [`jacobian_batch`]. It **must be a pure deterministic
///   function of its arguments**.
///
/// # Returns
///
/// A [`JacobianSolution`] whose [`matrix`](JacobianSolution::matrix) is `Some`
/// only if every column succeeded.
///
/// # Cost
///
/// [`DiffScheme::evaluations_per_jacobian`] calls to `f`: `n + 1` for
/// [`DiffScheme::Forward`]/[`DiffScheme::Backward`], `2n` for
/// [`DiffScheme::Central`], `4n` for [`DiffScheme::Central4th`].
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::differentiate::{jacobian, DiffSettings};
///
/// // f(x, y) = [x^2 * y, sin(x) + y^3]
/// // J = [[2xy, x^2], [cos(x), 3y^2]]
/// let point = [1.5_f64, 2.0];
/// let s = jacobian(
///     &point,
///     DiffSettings::central(),
///     ComputeBackend::Serial,
///     |_, v: &[f64], out: &mut Vec<f64>| {
///         out.push(v[0] * v[0] * v[1]);
///         out.push(v[0].sin() + v[1] * v[1] * v[1]);
///     },
/// );
///
/// let j = s.matrix().expect("smooth everywhere");
/// let (x, y) = (point[0], point[1]);
/// for (got, want) in [
///     (j.get(0, 0), 2.0 * x * y),
///     (j.get(0, 1), x * x),
///     (j.get(1, 0), x.cos()),
///     (j.get(1, 1), 3.0 * y * y),
/// ] {
///     assert!((got - want).abs() < 1e-8 * want.abs().max(1.0), "{got} vs {want}");
/// }
/// ```
#[must_use]
pub fn jacobian<F>(
    x: &[f64],
    settings: DiffSettings,
    backend: ComputeBackend,
    f: F,
) -> JacobianSolution
where
    F: Fn(usize, &[f64], &mut Vec<f64>) + Sync,
{
    jacobian_columns_min(0, x, settings, backend, JACOBIAN_COLUMN_MIN_DIMENSION, &f)
}

/// Assemble `N` independent Jacobians, one per lane, on the chosen backend.
///
/// This is the batched form: the parallel axis is the **lane**, not the column,
/// so it is the right entry point when there are many small Jacobians (a
/// per-cell chemistry Jacobian over a mesh, an ensemble of ODE systems). Use
/// [`jacobian`] when there is one large Jacobian instead.
///
/// # The flat point layout
///
/// `points` is a **flat, row-major** buffer of `lanes * n` values: lane `i`'s
/// point is `points[i * n .. (i + 1) * n]`. A `&[Vec<f64>]` would be the
/// obvious alternative and is rejected on purpose — it costs one allocation and
/// one pointer chase per lane, and it is not the layout a GPU buffer would ever
/// take. `points.len()` must be an exact multiple of `n`.
///
/// # Arguments
///
/// - `points` — flat `lanes * n` buffer as above, in the caller's own units.
/// - `n` — the dimension of each point, dimensionless. Must be non-zero.
/// - `settings` — scheme and step-size policy, shared by every lane.
/// - `backend` — requested backend; see [`jacobian_batch_backend_for`]. Each
///   lane's columns are computed serially, since the lane axis is already the
///   parallel one.
/// - `f` — `f(i, x, out)` must fill `out` with lane `i`'s `n` function
///   components at `x`. It **must be a pure deterministic function of its
///   arguments**.
///
/// # Returns
///
/// A [`JacobianBatch`] with one solution per lane, in lane order. An empty
/// `points` slice, or `n == 0`, returns an empty batch and calls `f` zero
/// times. A `points.len()` that is not a multiple of `n` returns an empty batch
/// as well — it is a caller bug, not a numerical failure, and there is no
/// sensible lane count to report per-lane statuses against.
///
/// # Cost
///
/// `lanes * `[`DiffScheme::evaluations_per_jacobian`]`(n)` calls to `f`.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::differentiate::{jacobian_batch, DiffSettings};
///
/// // 3 lanes of the 2-D rotation-like system f = [-k_i * y, k_i * x],
/// // whose Jacobian is [[0, -k_i], [k_i, 0]].
/// let k = [1.0_f64, 2.5, 7.0];
/// let points: Vec<f64> = vec![0.3, -0.7, 0.3, -0.7, 0.3, -0.7];
///
/// let batch = jacobian_batch(
///     &points,
///     2,
///     DiffSettings::central(),
///     ComputeBackend::CpuMulti,
///     |i, v: &[f64], out: &mut Vec<f64>| {
///         out.push(-k[i] * v[1]);
///         out.push(k[i] * v[0]);
///     },
/// );
///
/// let mats = batch.matrices().expect("linear system, exact everywhere");
/// for (i, m) in mats.iter().enumerate() {
///     assert!((m.get(0, 1) + k[i]).abs() < 1e-9);
///     assert!((m.get(1, 0) - k[i]).abs() < 1e-9);
/// }
/// ```
#[must_use]
pub fn jacobian_batch<F>(
    points: &[f64],
    n: usize,
    settings: DiffSettings,
    backend: ComputeBackend,
    f: F,
) -> JacobianBatch
where
    F: Fn(usize, &[f64], &mut Vec<f64>) + Sync,
{
    jacobian_batch_min(
        points,
        n,
        settings,
        backend,
        JACOBIAN_BATCH_MIN_PROBLEMS,
        f,
    )
}

/// [`jacobian_batch`] with the size floor supplied by the caller.
///
/// Exists so the crossover benchmark can measure the multi-CPU path *below*
/// [`JACOBIAN_BATCH_MIN_PROBLEMS`], and so the cross-backend bitwise tests are
/// not vacuous on small batches. Not public.
pub(crate) fn jacobian_batch_min<F>(
    points: &[f64],
    n: usize,
    settings: DiffSettings,
    backend: ComputeBackend,
    min_problems: usize,
    f: F,
) -> JacobianBatch
where
    F: Fn(usize, &[f64], &mut Vec<f64>) + Sync,
{
    if n == 0 || points.is_empty() || !points.len().is_multiple_of(n) {
        return JacobianBatch {
            solutions: Vec::new(),
        };
    }
    let lanes = points.len() / n;
    // Per-lane columns always run serially: the lane axis is the parallel one,
    // and nesting a rayon map inside a rayon map only fights the same pool.
    let solutions: Vec<JacobianSolution> = match effective_backend(backend, lanes, min_problems) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => points
            .par_chunks(n)
            .enumerate()
            .map(|(i, x)| {
                jacobian_columns_min(i, x, settings, ComputeBackend::Serial, usize::MAX, &f)
            })
            .collect(),
        _ => points
            .chunks(n)
            .enumerate()
            .map(|(i, x)| {
                jacobian_columns_min(i, x, settings, ComputeBackend::Serial, usize::MAX, &f)
            })
            .collect(),
    };
    JacobianBatch { solutions }
}

/// Assemble one lane's Jacobian, spreading the columns across `backend` when
/// the dimension justifies it.
///
/// Delegates to [`jacobian_columns_serial`] for everything except the `rayon`
/// arm, and both arms call the same [`jacobian_column`] kernel — which is why
/// every public Jacobian entry point agrees bit for bit.
fn jacobian_columns_min<F>(
    lane: usize,
    x: &[f64],
    settings: DiffSettings,
    backend: ComputeBackend,
    min_dimension: usize,
    f: &F,
) -> JacobianSolution
where
    F: Fn(usize, &[f64], &mut Vec<f64>) + Sync,
{
    match effective_backend(backend, x.len(), min_dimension) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => {
            let n = x.len();
            let base = match jacobian_base(lane, x, settings, f) {
                Ok(b) => b,
                Err(status) => return failed_jacobian(n, status, 0),
            };
            if !x.iter().all(|v| v.is_finite()) {
                return failed_jacobian(n, DiffStatus::InvalidPoint, 0);
            }
            let base_slice = base.as_deref();
            let columns: Vec<Result<Vec<f64>, DiffStatus>> = (0..n)
                .into_par_iter()
                .map(|j| jacobian_column(lane, x, j, settings, base_slice, f))
                .collect();
            assemble_columns(n, columns)
        }
        _ => jacobian_columns_serial(lane, x, settings, f),
    }
}

/// One lane's Jacobian, columns computed on the calling thread.
///
/// Carries **no** `Sync` bound, which is what lets [`ode_system_jacobian`]
/// difference an `OdeSystem` that is not itself `Sync`.
fn jacobian_columns_serial<F>(
    lane: usize,
    x: &[f64],
    settings: DiffSettings,
    f: &F,
) -> JacobianSolution
where
    F: Fn(usize, &[f64], &mut Vec<f64>),
{
    let n = x.len();
    if n == 0 {
        return JacobianSolution {
            matrix: SquareMatrix::new(0),
            status: DiffStatus::Ok,
            first_bad_column: usize::MAX,
        };
    }
    if !x.iter().all(|v| v.is_finite()) {
        return failed_jacobian(n, DiffStatus::InvalidPoint, 0);
    }
    let base = match jacobian_base(lane, x, settings, f) {
        Ok(b) => b,
        Err(status) => return failed_jacobian(n, status, 0),
    };
    let base_slice = base.as_deref();
    let columns: Vec<Result<Vec<f64>, DiffStatus>> = (0..n)
        .map(|j| jacobian_column(lane, x, j, settings, base_slice, f))
        .collect();
    assemble_columns(n, columns)
}

/// The unperturbed evaluation `f(x)`, shared by every column of a one-sided
/// scheme — which is what makes [`DiffScheme::Forward`] cost `n + 1`
/// evaluations rather than `2n`. `None` for the symmetric schemes, whose
/// stencils never touch `x` itself.
fn jacobian_base<F>(
    lane: usize,
    x: &[f64],
    settings: DiffSettings,
    f: &F,
) -> Result<Option<Vec<f64>>, DiffStatus>
where
    F: Fn(usize, &[f64], &mut Vec<f64>),
{
    match settings.scheme {
        DiffScheme::Forward | DiffScheme::Backward => {
            let n = x.len();
            let mut out = Vec::with_capacity(n);
            f(lane, x, &mut out);
            if out.len() != n {
                return Err(DiffStatus::DimensionMismatch);
            }
            if !out.iter().all(|v| v.is_finite()) {
                return Err(DiffStatus::NotFinite);
            }
            Ok(Some(out))
        }
        DiffScheme::Central | DiffScheme::Central4th => Ok(None),
    }
}

/// Pack per-column results into a [`SquareMatrix`], filling failed columns with
/// `NaN` and recording the first failure.
fn assemble_columns(n: usize, columns: Vec<Result<Vec<f64>, DiffStatus>>) -> JacobianSolution {
    let mut matrix = SquareMatrix::new(n);
    let mut status = DiffStatus::Ok;
    let mut first_bad_column = usize::MAX;
    for (j, column) in columns.into_iter().enumerate() {
        match column {
            Ok(values) => {
                for (i, v) in values.into_iter().enumerate() {
                    matrix.set(i, j, v);
                }
            }
            Err(bad) => {
                for i in 0..n {
                    matrix.set(i, j, f64::NAN);
                }
                if status.is_ok() {
                    status = bad;
                    first_bad_column = j;
                }
            }
        }
    }
    JacobianSolution {
        matrix,
        status,
        first_bad_column,
    }
}

/// An all-`NaN` Jacobian carrying the reason it could not be formed.
fn failed_jacobian(n: usize, status: DiffStatus, first_bad_column: usize) -> JacobianSolution {
    let mut matrix = SquareMatrix::new(n);
    for i in 0..n {
        for j in 0..n {
            matrix.set(i, j, f64::NAN);
        }
    }
    JacobianSolution {
        matrix,
        status,
        first_bad_column,
    }
}

/// Column `j` of the Jacobian: `d f / d x_j`, all `n` components at once.
///
/// `base` carries the shared unperturbed evaluation for the one-sided schemes
/// and is `None` for the symmetric ones.
#[inline]
fn jacobian_column<F>(
    lane: usize,
    x: &[f64],
    j: usize,
    settings: DiffSettings,
    base: Option<&[f64]>,
    f: &F,
) -> Result<Vec<f64>, DiffStatus>
where
    F: Fn(usize, &[f64], &mut Vec<f64>),
{
    let n = x.len();
    let h = settings.step_for(x[j]);
    if !h.is_finite() || h <= 0.0 {
        return Err(DiffStatus::DegenerateStep);
    }

    match settings.scheme {
        DiffScheme::Forward => one_sided_column(lane, x, j, h, base, f),
        DiffScheme::Backward => one_sided_column(lane, x, j, -h, base, f),
        DiffScheme::Central => central_column(lane, x, j, h, f),
        DiffScheme::Central4th => {
            let coarse = central_column(lane, x, j, h, f)?;
            let fine = central_column(lane, x, j, 0.5 * h, f)?;
            let mut out = Vec::with_capacity(n);
            for (c, fi) in coarse.into_iter().zip(fine) {
                let v = (4.0 * fi - c) / 3.0;
                if !v.is_finite() {
                    return Err(DiffStatus::NotFinite);
                }
                out.push(v);
            }
            Ok(out)
        }
    }
}

/// Forward (`h > 0`) or backward (`h < 0`) column, reusing the shared base.
#[inline]
fn one_sided_column<F>(
    lane: usize,
    x: &[f64],
    j: usize,
    h: f64,
    base: Option<&[f64]>,
    f: &F,
) -> Result<Vec<f64>, DiffStatus>
where
    F: Fn(usize, &[f64], &mut Vec<f64>),
{
    let n = x.len();
    let base = base.ok_or(DiffStatus::DegenerateStep)?;
    let mut xp = x.to_vec();
    xp[j] = x[j] + h;
    let dh = xp[j] - x[j];
    if dh == 0.0 || !dh.is_finite() {
        return Err(DiffStatus::DegenerateStep);
    }
    let mut fp = Vec::with_capacity(n);
    f(lane, &xp, &mut fp);
    if fp.len() != n {
        return Err(DiffStatus::DimensionMismatch);
    }
    let mut out = Vec::with_capacity(n);
    for (a, b) in fp.into_iter().zip(base) {
        let v = (a - b) / dh;
        if !v.is_finite() {
            return Err(DiffStatus::NotFinite);
        }
        out.push(v);
    }
    Ok(out)
}

/// Central column with half-width `h`.
#[inline]
fn central_column<F>(
    lane: usize,
    x: &[f64],
    j: usize,
    h: f64,
    f: &F,
) -> Result<Vec<f64>, DiffStatus>
where
    F: Fn(usize, &[f64], &mut Vec<f64>),
{
    let n = x.len();
    // One probe buffer, reused for both stencil points -- the two evaluations
    // differ in a single component, so a second copy of `x` buys nothing.
    let mut probe = x.to_vec();
    probe[j] = x[j] + h;
    let plus = probe[j];
    probe[j] = x[j] - h;
    let minus = probe[j];
    let dh = plus - minus;
    if dh == 0.0 || !dh.is_finite() {
        return Err(DiffStatus::DegenerateStep);
    }
    let mut fm = Vec::with_capacity(n);
    f(lane, &probe, &mut fm);
    probe[j] = plus;
    let mut fp = Vec::with_capacity(n);
    f(lane, &probe, &mut fp);
    if fp.len() != n || fm.len() != n {
        return Err(DiffStatus::DimensionMismatch);
    }
    let mut out = Vec::with_capacity(n);
    for (a, b) in fp.into_iter().zip(fm) {
        let v = (a - b) / dh;
        if !v.is_finite() {
            return Err(DiffStatus::NotFinite);
        }
        out.push(v);
    }
    Ok(out)
}

// ── The ODE consumer ─────────────────────────────────────────────────────────

/// Fill an [`OdeSystem`]'s Jacobian slots by finite differences.
///
/// This is the free-function form of what [`NumericalJacobian`] does, for
/// callers who already have an `OdeSystem` and want the numbers rather than a
/// wrapper. It fills exactly the two buffers
/// [`OdeSystem::jacobian`] is contracted to fill:
///
/// - `dfdy[i][j] = d f_i / d y_j`, an `n x n` [`SquareMatrix`];
/// - `dfdx[i] = d f_i / d x`, the derivative with respect to the **independent
///   variable** (time, for a transient), length `n`.
///
/// Both are resized to `n = system.n_eqns()` if the caller's buffers are the
/// wrong size, so it is safe to pass freshly-defaulted ones.
///
/// # Failure is written into the buffers, not swallowed
///
/// On failure the offending entries are filled with `NaN` **and** the reason is
/// returned. Nothing is quietly zeroed. This matters: filling a failed entry
/// with `0.0` — which
/// `outram-park-fork-dwsim-libs`' `finite_difference_jacobian` does — turns an
/// undetectable-at-the-call-site failure into a plausible-looking Jacobian, and
/// a Rosenbrock step built on it produces a wrong trajectory rather than an
/// error. With `NaN` the failure propagates into the step, the normalised error
/// estimate becomes `NaN`, the step controller shrinks `dx` and
/// [`crate::ode::OdeError::StepSizeUnderflow`] is reported. Loud is better.
///
/// # Arguments
///
/// - `system` — the ODE system whose [`OdeSystem::derivatives`] is sampled.
/// - `x` — the independent variable, caller's units.
/// - `y` — the state, length `system.n_eqns()`.
/// - `settings` — scheme and step-size policy; see [`DiffSettings`].
/// - `dfdx`, `dfdy` — output buffers, filled in place.
///
/// # Returns
///
/// [`DiffStatus::Ok`] if every entry of both outputs was formed from finite
/// evaluations; the first failing status otherwise.
///
/// # Cost
///
/// `1 + `[`DiffScheme::evaluations_per_jacobian`]`(n)` calls to
/// [`OdeSystem::derivatives`] for the one-sided schemes — the base evaluation
/// is shared between `dfdy`'s columns and `dfdx` — and
/// [`DiffScheme::evaluations_per_jacobian`]`(n) + 2` or `+ 4` for the symmetric
/// ones.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::math::differentiate::{
///     ode_system_jacobian, DiffSettings, DiffStatus,
/// };
/// use outram_foam_basic_lib::matrix::SquareMatrix;
/// use outram_foam_basic_lib::ode::OdeSystem;
///
/// // dy/dx = [-2*y0 + y1, x * y0] -- Jacobian [[-2, 1], [x, 0]], dfdx = [0, y0].
/// struct Linear;
/// impl OdeSystem for Linear {
///     fn n_eqns(&self) -> usize { 2 }
///     fn derivatives(&self, x: f64, y: &[f64], dydx: &mut Vec<f64>) {
///         dydx.clear();
///         dydx.push(-2.0 * y[0] + y[1]);
///         dydx.push(x * y[0]);
///     }
/// }
///
/// let mut dfdx = Vec::new();
/// let mut dfdy = SquareMatrix::new(2);
/// let status = ode_system_jacobian(
///     &Linear, 0.5, &[1.0, 2.0], DiffSettings::central(), &mut dfdx, &mut dfdy,
/// );
///
/// assert_eq!(status, DiffStatus::Ok);
/// assert!((dfdy.get(0, 0) + 2.0).abs() < 1e-8);
/// assert!((dfdy.get(0, 1) - 1.0).abs() < 1e-8);
/// assert!((dfdy.get(1, 0) - 0.5).abs() < 1e-8);
/// assert!(dfdy.get(1, 1).abs() < 1e-8);
/// assert!(dfdx[0].abs() < 1e-8);
/// assert!((dfdx[1] - 1.0).abs() < 1e-8); // d/dx (x * y0) = y0 = 1
/// ```
pub fn ode_system_jacobian<S>(
    system: &S,
    x: f64,
    y: &[f64],
    settings: DiffSettings,
    dfdx: &mut Vec<f64>,
    dfdy: &mut SquareMatrix,
) -> DiffStatus
where
    S: OdeSystem + ?Sized,
{
    let n = system.n_eqns();
    if dfdx.len() != n {
        dfdx.resize(n, 0.0);
    }
    if dfdy.n() != n {
        *dfdy = SquareMatrix::new(n);
    }

    // d f_i / d y_j -- the state Jacobian. Columns run serially: an ODE
    // Jacobian is small (n is equation count, typically single or double
    // digits) and `ode::parallel::integrate_ensemble` already provides the
    // parallel axis that matters for ODE work, which is the ensemble lane.
    let solution = jacobian_columns_serial(
        0,
        y,
        settings,
        &|_: usize, state: &[f64], out: &mut Vec<f64>| {
            out.clear();
            system.derivatives(x, state, out);
        },
    );
    for i in 0..n {
        for j in 0..n {
            dfdy.set(i, j, solution.raw_matrix().get(i, j));
        }
    }
    let mut status = solution.status();

    // d f_i / d x -- the explicit dependence on the independent variable.
    // Same stencil, same step rule, one scalar direction.
    let x_status = ode_dfdx(system, x, y, settings, n, dfdx);
    if status.is_ok() {
        status = x_status;
    }
    status
}

/// Fill `dfdx` — the derivative of each equation with respect to the
/// independent variable, holding the state fixed.
fn ode_dfdx<S>(
    system: &S,
    x: f64,
    y: &[f64],
    settings: DiffSettings,
    n: usize,
    dfdx: &mut [f64],
) -> DiffStatus
where
    S: OdeSystem + ?Sized,
{
    // Treat `x` as a one-component point and reuse the column kernel, so the
    // stencil, the step rule and the realised-step correction are literally the
    // same code as for `dfdy`.
    let point = [x];
    let solution = jacobian_column_scalar_direction(&point, settings, n, &|t: f64,
                                                                           out: &mut Vec<f64>| {
        out.clear();
        system.derivatives(t, y, out);
    });
    match solution {
        Ok(values) => {
            dfdx[..n].copy_from_slice(&values[..n]);
            DiffStatus::Ok
        }
        Err(status) => {
            for v in dfdx.iter_mut().take(n) {
                *v = f64::NAN;
            }
            status
        }
    }
}

/// The `dfdx` stencil: one scalar direction, `n` outputs.
///
/// Structurally identical to [`jacobian_column`] with `j = 0`; kept separate
/// only because its function takes a bare `f64` rather than a slice, which is
/// what [`OdeSystem::derivatives`] wants for its independent variable.
fn jacobian_column_scalar_direction<G>(
    point: &[f64; 1],
    settings: DiffSettings,
    n: usize,
    g: &G,
) -> Result<Vec<f64>, DiffStatus>
where
    G: Fn(f64, &mut Vec<f64>),
{
    let x = point[0];
    if !x.is_finite() {
        return Err(DiffStatus::InvalidPoint);
    }
    let h = settings.step_for(x);
    if !h.is_finite() || h <= 0.0 {
        return Err(DiffStatus::DegenerateStep);
    }

    let sample = |t: f64| -> Result<Vec<f64>, DiffStatus> {
        let mut out = Vec::with_capacity(n);
        g(t, &mut out);
        if out.len() != n {
            return Err(DiffStatus::DimensionMismatch);
        }
        if out.iter().any(|v| !v.is_finite()) {
            return Err(DiffStatus::NotFinite);
        }
        Ok(out)
    };
    let quotient = |a: Vec<f64>, b: Vec<f64>, dh: f64| -> Result<Vec<f64>, DiffStatus> {
        let mut out = Vec::with_capacity(n);
        for (p, q) in a.into_iter().zip(b) {
            let v = (p - q) / dh;
            if !v.is_finite() {
                return Err(DiffStatus::NotFinite);
            }
            out.push(v);
        }
        Ok(out)
    };
    let one_sided = |step: f64| -> Result<Vec<f64>, DiffStatus> {
        let xp = x + step;
        let dh = xp - x;
        if dh == 0.0 || !dh.is_finite() {
            return Err(DiffStatus::DegenerateStep);
        }
        quotient(sample(xp)?, sample(x)?, dh)
    };
    let central = |half: f64| -> Result<Vec<f64>, DiffStatus> {
        let (xp, xm) = (x + half, x - half);
        let dh = xp - xm;
        if dh == 0.0 || !dh.is_finite() {
            return Err(DiffStatus::DegenerateStep);
        }
        quotient(sample(xp)?, sample(xm)?, dh)
    };

    match settings.scheme {
        DiffScheme::Forward => one_sided(h),
        DiffScheme::Backward => one_sided(-h),
        DiffScheme::Central => central(h),
        DiffScheme::Central4th => {
            let coarse = central(h)?;
            let fine = central(0.5 * h)?;
            let mut out = Vec::with_capacity(n);
            for (c, fi) in coarse.into_iter().zip(fine) {
                let v = (4.0 * fi - c) / 3.0;
                if !v.is_finite() {
                    return Err(DiffStatus::NotFinite);
                }
                out.push(v);
            }
            Ok(out)
        }
    }
}

/// Wrap any [`OdeSystem`] so that [`crate::ode::Rosenbrock23`] can integrate it
/// **without a hand-coded Jacobian**.
///
/// # The problem this solves
///
/// [`OdeSystem::jacobian`] has a default body that is `unimplemented!()`, so a
/// system that does not override it panics the moment a stiff solver asks for a
/// Jacobian — inside `Rosenbrock23::inner_step`, and, if the integration is
/// running in an ensemble, out through the `rayon` scope. Every system that
/// only knows its own `derivatives` is locked out of the crate's only stiff
/// solver.
///
/// Wrapping it in `NumericalJacobian` supplies the missing method by finite
/// differences and changes nothing else: `n_eqns` and `derivatives` are
/// forwarded verbatim.
///
/// # Owning, not borrowing
///
/// The wrapper **owns** the system by value, so it needs no lifetime parameter
/// and no `Box` — both forbidden by the workspace design rules. Construct it
/// with [`Self::new`], get the system back with [`Self::into_inner`].
///
/// # An analytic Jacobian is still better
///
/// Finite differences cost `n + 1` to `4n` extra `derivatives` calls per
/// Rosenbrock stage and are accurate to roughly `sqrt(eps)` to `eps^(4/5)`
/// rather than to machine precision — see the module-level "Achievable
/// accuracy" table. If the analytic Jacobian is available, write it. This
/// wrapper is for the systems where it is not, and as a **verification oracle**
/// for the ones where it is: differencing a system that also implements
/// `jacobian` analytically and comparing is the cheapest real check that the
/// hand-derived version has no sign or transposition error.
///
/// # Units
///
/// Inherited from the wrapped system; nothing here is dimensioned.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::math::differentiate::{DiffSettings, NumericalJacobian};
/// use outram_foam_basic_lib::ode::{OdeSystem, Rosenbrock23};
///
/// // A stiff scalar system with NO hand-coded Jacobian: dy/dx = -1000 y.
/// struct StiffDecay;
/// impl OdeSystem for StiffDecay {
///     fn n_eqns(&self) -> usize { 1 }
///     fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
///         dydx.clear();
///         dydx.push(-1000.0 * y[0]);
///     }
///     // no `jacobian` override -- the default would panic
/// }
///
/// let system = NumericalJacobian::new(StiffDecay, DiffSettings::central());
/// let mut solver = Rosenbrock23::new(1, 1e-10, 1e-10);
/// let mut y = vec![1.0_f64];
/// let mut dx = 1e-5;
/// solver.integrate(&system, 0.0, 0.01, &mut y, &mut dx).expect("integrates");
///
/// // exp(-1000 * 0.01) = exp(-10) = 4.5399929762484854e-5
/// let exact = (-10.0_f64).exp();
/// assert!((y[0] - exact).abs() < 1e-8, "got {}, want {exact}", y[0]);
/// assert_eq!(system.non_finite_jacobians(), 0);
/// ```
#[derive(Debug)]
pub struct NumericalJacobian<S> {
    system: S,
    settings: DiffSettings,
    non_finite: AtomicUsize,
}

impl<S> NumericalJacobian<S> {
    /// Wrap `system`, differencing its `derivatives` with `settings`.
    ///
    /// # Arguments
    ///
    /// - `system` — owned by value.
    /// - `settings` — scheme and step-size policy. [`DiffSettings::central`] is
    ///   the usual choice for a Rosenbrock Jacobian: `2n` evaluations for
    ///   `O(h^2)` truncation, where [`DiffScheme::Central4th`] doubles the cost
    ///   again for accuracy the step controller cannot exploit.
    pub fn new(system: S, settings: DiffSettings) -> Self {
        Self {
            system,
            settings,
            non_finite: AtomicUsize::new(0),
        }
    }

    /// Borrow the wrapped system.
    pub fn inner(&self) -> &S {
        &self.system
    }

    /// Unwrap, returning the system.
    pub fn into_inner(self) -> S {
        self.system
    }

    /// The step-size policy in force.
    #[must_use]
    pub fn settings(&self) -> DiffSettings {
        self.settings
    }

    /// How many [`OdeSystem::jacobian`] calls have failed since construction.
    ///
    /// The trait method returns `()`, so it has nowhere to report a failure;
    /// this counter is that report. A non-zero value means at least one
    /// Jacobian was handed to the solver with `NaN` entries, which the solver
    /// will have turned into [`crate::ode::OdeError::StepSizeUnderflow`] rather
    /// than a wrong answer — but it is worth knowing *why* an integration
    /// failed, and "the Jacobian could not be differenced" is a different bug
    /// from "the system is too stiff".
    ///
    /// # Units
    ///
    /// A count, dimensionless.
    #[must_use]
    pub fn non_finite_jacobians(&self) -> usize {
        self.non_finite.load(Ordering::Relaxed)
    }
}

impl<S> OdeSystem for NumericalJacobian<S>
where
    S: OdeSystem,
{
    fn n_eqns(&self) -> usize {
        self.system.n_eqns()
    }

    fn derivatives(&self, x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        self.system.derivatives(x, y, dydx);
    }

    fn jacobian(&self, x: f64, y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
        let status = ode_system_jacobian(&self.system, x, y, self.settings, dfdx, dfdy);
        if !status.is_ok() {
            self.non_finite.fetch_add(1, Ordering::Relaxed);
        }
    }
}
