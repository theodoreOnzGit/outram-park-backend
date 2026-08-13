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

//! Batched **numerical integration** on the hybrid execution backend — `N`
//! independent initial-value problems, or `N` independent definite integrals,
//! advanced at once, serially or across CPU cores.
//!
//! # The two things in here, and why they share a module
//!
//! | Operation | The batch is | Entry point |
//! |---|---|---|
//! | ODE ensemble, one stepper for every lane | `N` independent IVPs | [`integrate_ensemble`] |
//! | ODE ensemble, stepper chosen per lane | `N` independent IVPs | [`integrate_ensemble_mixed`] |
//! | Fixed-rule quadrature | `N` independent definite integrals | [`quadrature_batch`] |
//! | Adaptive quadrature | `N` independent definite integrals | [`adaptive_quadrature_batch`] |
//!
//! Both halves are "numerical integration" in the sense bead `op-yvj.4.5` uses
//! the phrase, and a definite integral genuinely *is* the initial-value problem
//! `dy/dx = f(x)`, `y(a) = 0`, read at `x = b` — so quadrature sitting beside the
//! ODE steppers is not a filing accident. They also share every piece of
//! machinery that is not the arithmetic itself: the lane vocabulary, the
//! per-lane status reporting, the backend-degradation policy, and the
//! determinism argument below. Keeping them together means the crate has **one**
//! batched-integration dialect rather than two.
//!
//! # Reuse, not reimplementation
//!
//! **No new integrator is written here.** The ODE half drives the crate's
//! existing steppers — [`Euler`](crate::ode::Euler),
//! [`Rkf45`](crate::ode::Rkf45), [`Rosenbrock23`](crate::ode::Rosenbrock23),
//! selected through the existing [`OdeSolver`] enum — over the existing
//! adaptive interval loop. This module adds the *ensemble*: the outer loop over
//! lanes, the per-lane outcome reporting, and the backend dispatch.
//!
//! The quadrature half is new code, because the workspace had no general
//! quadrature. The only prior art is
//! `outram-park-fork-dwsim-libs`'s `clean_energies::pem_fuel_cell::simpson_integrate`,
//! which is deliberately **not** reused: it is a verbatim port of OPEM's rule
//! *including its documented flaw* (composite Simpson weights applied without
//! checking that the sample count is odd), and it integrates a slice of
//! pre-sampled values rather than a callable integrand. [`QuadratureRule`]
//! keeps the one convention that does carry over — composite rules over equal
//! subintervals — and makes the sample-count error structurally impossible by
//! counting Simpson *panels* of two subintervals each rather than raw samples.
//! `raffles`'s `distributions::special::integrate_open_unit` is the other
//! in-workspace quadrature: a composite 8-point Gauss-Legendre over
//! geometrically graded panels, hard-wired to the open unit interval and to
//! quantile-function moments. It is not a general routine and is private to that
//! crate's `special` module, so it is cited here as precedent for the
//! Gauss-Legendre choice rather than reused.
//!
//! # Hybrid means dispatch, not two APIs
//!
//! Every entry point takes a [`ComputeBackend`] parameter, and there is no
//! `*_parallel()` sibling anywhere. With the `parallel` feature off,
//! [`ComputeBackend::CpuMulti`] resolves down to [`ComputeBackend::Serial`]
//! through [`ComputeBackend::resolve`] and the answer is unchanged, bit for bit.
//! There is no GPU kernel in this module yet, so a [`ComputeBackend::Gpu`]
//! request degrades to the best available CPU path — see "GPU" below for why
//! that is not merely laziness for the adaptive paths.
//!
//! # Determinism — bitwise identical, and the summation-order question
//!
//! **Every kernel in this module returns bit-for-bit identical output on
//! [`ComputeBackend::Serial`] and [`ComputeBackend::CpuMulti`], at any thread
//! count, on every run**, provided the caller's system or integrand is itself a
//! deterministic pure function of its arguments.
//!
//! For the ODE ensemble this is the same argument as
//! [`crate::math::parallel`]'s: lane `i`'s trajectory is a pure function of lane
//! `i`'s system, initial condition and stepper. No lane reads another lane's
//! state, so there is no cross-lane arithmetic whose association could change,
//! and both backends call the very same per-lane kernel. Each lane also gets its
//! **own** clone of the stepper prototype, so no scratch buffer is ever shared
//! between lanes and the result cannot depend on which lanes a worker happened
//! to run first.
//!
//! Quadrature needs one extra sentence, because a quadrature rule *is* a sum and
//! floating-point addition is not associative. The reason the answer is still
//! bit-identical is the shape of the batch: **one lane is one integral, and one
//! integral is summed sequentially by a single thread.** The parallelism is over
//! lanes, never within a lane, so no partial sum is ever re-associated. This is
//! a deliberate design choice with a real cost — it means a single very
//! expensive integral gets no speed-up from this module at all — and it is taken
//! because a reduction split across threads would give a different answer at
//! every thread count, which for a verification oracle is disqualifying.
//! **Splitting one integral's panels across threads is not offered here**, and
//! if it is ever added it must be a separate, separately-named entry point whose
//! documentation says plainly that it is not bit-reproducible.
//!
//! Verified by the `bitwise_*` tests in `parallel/tests.rs`, which compare
//! serial against `rayon` pools of 1, 2, 4 and 8 workers on batches built to
//! have wildly uneven per-lane cost.
//!
//! For scale, a 4 096-lane deliberately-imbalanced ensemble (half the lanes
//! decaying at `k` near 1, half at `k` near 60, so accepted step counts run from
//! about 10 to 126 per lane, 278 318 in total) under `Rkf45`, measured
//! 2026-08-13 on 4 logical cores by the `#[ignore]`d
//! `ensemble_thread_scaling_benchmark`, best of 7 samples, with a second
//! independent run alongside:
//!
//! | Worker threads | Time | Speed-up | (repeat) | Bitwise vs serial |
//! |---|---|---|---|---|
//! | *serial reference* | 35808.33 us | 1.00x | 1.00x | — |
//! | 1 | 35839.25 us | 1.00x | 1.00x | identical |
//! | 2 | 18283.82 us | 1.96x | 1.92x | identical |
//! | 4 | 9276.65 us | 3.86x | 3.09x | identical |
//! | 8 | 9128.96 us | 3.92x | 3.05x | identical |
//!
//! The "identical" column is the determinism claim above measured rather than
//! argued, and it is asserted by the benchmark itself, not merely printed. Going
//! through `rayon` with a single worker costs essentially nothing here (1.00x),
//! unlike the batched root finder where it costs about 6% — the per-lane work is
//! large enough that the iterator machinery disappears into it. Eight workers on
//! four cores buy nothing further, the expected signature of a compute-bound
//! kernel that already saturates its cores. **This is one machine, one ensemble
//! and two runs; it is not a scaling study**, and nothing here has been measured
//! on Android hardware or on a many-core server.
//!
//! # Load imbalance — why there is no hand-rolled partition
//!
//! An adaptive stepper takes a different number of sub-steps in every lane, and
//! the spread is not small: a stiff lane under [`Rkf45`](crate::ode::Rkf45) can
//! take thousands of steps while its neighbour takes twelve. Adaptive quadrature
//! has exactly the same property, by construction. A static equal split across
//! `P` threads therefore ends up waiting on whichever chunk drew the hard lanes.
//!
//! Every parallel path here uses `rayon`'s adaptive splitting with **no**
//! `min_len` floor, so an idle worker can steal down to a single lane. That is
//! the deliberate answer to the imbalance, not an oversight. No granularity
//! floor is imposed even on the fixed-rule quadrature path, where the crate's
//! closed-form polynomial kernels do impose one
//! ([`crate::math::parallel::POLY_BLOCK`]): there, every lane provably costs the
//! same handful of flops, whereas here the per-lane cost is set by the caller
//! through [`QuadratureRule`]'s panel count *and* by the cost of the caller's
//! integrand, so any fixed floor would be wrong for most callers. Work-stealing
//! handles both ends without a number that cannot be justified.
//!
//! Whatever the splitter does, it cannot change a value — every lane is computed
//! independently of every other.
//!
//! # Stiffness — how a mixed ensemble is handled
//!
//! The realistic per-cell chemistry ensemble is *mixed*: most cells are benign
//! and a few are stiff. Three things follow, and all three are deliberate.
//!
//! 1. **The stepper is not switched behind the caller's back.** An ensemble run
//!    with [`integrate_ensemble`] uses one stepper for every lane. A stiff lane
//!    handed an explicit stepper does not silently return a wrong answer: the
//!    adaptive controller shrinks the step until it either meets tolerance —
//!    correct but slow — or runs out, at which point the lane reports
//!    [`OdeLaneStatus::MaxStepsExceeded`] or
//!    [`OdeLaneStatus::StepSizeUnderflow`]. Stiffness therefore shows up as a
//!    *named per-lane failure*, not as silent garbage.
//! 2. **Per-lane stepper selection exists** — [`integrate_ensemble_mixed`] takes
//!    a closure `Fn(usize) -> OdeSolver`, so a caller that knows which cells are
//!    stiff (or that has just been told by a failed
//!    [`integrate_ensemble`] pass) can give those lanes
//!    [`Rosenbrock23`](crate::ode::Rosenbrock23) and leave the rest on
//!    [`Rkf45`](crate::ode::Rkf45). This is the intended recovery path and it
//!    costs nothing when unused.
//! 3. **`Rosenbrock23` needs a Jacobian, and this module does not supply one.**
//!    [`OdeSystem::jacobian`](crate::ode::OdeSystem::jacobian)'s default
//!    implementation panics, and a panic inside a `rayon` worker will propagate
//!    out of the batch. Check
//!    [`OdeSolver::requires_jacobian`](crate::ode::OdeSolver::requires_jacobian)
//!    before selecting it for a system you did not write. Batched *numerical*
//!    Jacobians are bead `op-yvj.4.6`, not this one.
//!
//! # Non-convergence is reported, never swallowed
//!
//! An ensemble of 10 000 cells in which 3 fail must say so, and must say which
//! 3. Both halves report failure **per lane**, and make it hard to ignore by
//! construction — the same shape [`crate::math::parallel`] uses:
//!
//! - [`OdeLaneSolution::state`] and [`QuadratureSolution::value`] return
//!   `Option`, and hand back `Some` **only** for a lane that succeeded.
//! - The raw number is behind the deliberately-named
//!   [`OdeLaneSolution::last_state`] / [`QuadratureSolution::last_value`], so
//!   using a failed lane's partial answer is a visible decision in the calling
//!   code rather than an accident.
//! - [`OdeEnsemble::states`] and [`QuadratureBatch::values`] are all-or-nothing:
//!   they return `Err` naming the failure count and the first failing lane,
//!   rather than a plausible-looking `Vec`.
//!
//! **A lane that ran out of steps is never presented as if it had reached
//! `x_end`.** It reports [`OdeLaneStatus::MaxStepsExceeded`], its genuine
//! partial state, and the `x` it actually reached — which is exactly the
//! information needed to decide whether to sub-cycle it or re-run it stiff. A
//! lane whose inputs were unusable reports [`OdeLaneStatus::InvalidLane`] and a
//! `NaN` state, because there is no honest number to return.
//!
//! # GPU
//!
//! There is no `wgpu` kernel here yet; `Gpu` degrades. Two of these four
//! kernels would map to one if written, and two would not, and the distinction
//! is worth recording rather than rediscovering:
//!
//! - **Fixed-rule quadrature is GPU-shaped.** Every lane evaluates the same
//!   fixed number of nodes with no data-dependent branching, so a warp never
//!   diverges.
//! - **Adaptive quadrature is not, and is CPU-only by design.** Its subdivision
//!   pattern is decided by the integrand, so neighbouring lanes take different
//!   paths through the code and different numbers of evaluations; on SIMT
//!   hardware that serialises the divergent branches and needs a per-lane
//!   work stack. It stays on the CPU deliberately.
//! - **Adaptive ODE ensembles have the same problem, sharpened.** A batch run
//!   lockstep at the smallest step any lane needs is simple and correct but
//!   wastes the whole ensemble's budget on its worst member; per-lane step
//!   control is correct but divergent. Neither is attempted here.
//!
//! Note also that WGSL has no `f64`, and the accuracy consequences of an `f32`
//! quadrature or an `f32` error controller are unmeasured in this workspace.
//!
//! # Units
//!
//! Everything here is dimensionless `f64`, for the same reason
//! [`crate::math::parallel`] is: a general integrator has no single physical
//! dimension. One lane's abscissa may be a time in seconds and another's a
//! length in metres, and the state vector's components routinely carry different
//! dimensions from each other.
//!
//! `uom` typing is **not stripped** to get here — it is applied at the boundary,
//! by the caller, exactly as the hybrid-backend epic requires: convert into the
//! batch, convert back out. The doctests on [`integrate_ensemble`] and
//! [`quadrature_batch`] show that boundary explicitly, one recovering a
//! `ThermodynamicTemperature` and the other an `Energy`.
//!
//! # Cargo features and portability
//!
//! The `rayon` paths sit behind the crate's `parallel` feature, which is **off
//! by default**; with it off this module still compiles and every entry point
//! still works. `rayon` is pure Rust with no system component, so everything
//! here compiles and runs on `aarch64-linux-android` / Termux exactly as on
//! desktop. Nothing in this module is target-gated.
//!
//! # Example — an ensemble of independent decays
//!
//! ```rust
//! use outram_foam_basic_lib::compute::ComputeBackend;
//! use outram_foam_basic_lib::ode::{OdeSolver, OdeSystem};
//! use outram_foam_basic_lib::ode::parallel::{integrate_ensemble, OdeLane};
//!
//! /// `dy/dx = -k y` — one lane per decay constant.
//! struct Decay { k: f64 }
//! impl OdeSystem for Decay {
//!     fn n_eqns(&self) -> usize { 1 }
//!     fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
//!         dydx[0] = -self.k * y[0];
//!     }
//! }
//!
//! let lanes: Vec<OdeLane<Decay>> = (1..=4)
//!     .map(|i| OdeLane::new(Decay { k: i as f64 }, vec![1.0], 0.0, 1.0, 0.1))
//!     .collect();
//!
//! let ensemble = integrate_ensemble(
//!     &lanes,
//!     &OdeSolver::rkf45(1, 1e-10, 1e-8),
//!     ComputeBackend::CpuMulti,
//! );
//!
//! let states = ensemble.states().expect("all four lanes complete");
//! for (i, s) in states.iter().enumerate() {
//!     let exact = (-(i as f64 + 1.0)).exp();
//!     assert!((s[0] - exact).abs() < 1e-8, "lane {i}: {} vs {exact}", s[0]);
//! }
//!
//! // Asking for multi-CPU gives a bit-for-bit identical answer, whether or not
//! // the `parallel` feature is compiled in.
//! let serial = integrate_ensemble(
//!     &lanes,
//!     &OdeSolver::rkf45(1, 1e-10, 1e-8),
//!     ComputeBackend::Serial,
//! );
//! assert_eq!(states, serial.states().unwrap());
//! ```

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{OdeError, OdeSolver, OdeSystem};
use crate::compute::ComputeBackend;

#[cfg(test)]
mod tests;

// ── Tuning constants ─────────────────────────────────────────────────────────

/// Lane count below which a [`ComputeBackend::CpuMulti`] request runs
/// [`integrate_ensemble`] on the calling thread instead.
///
/// # Measured crossover
///
/// *Methodology.* Measured 2026-08-13 on this workspace's development machine,
/// `std::thread::available_parallelism()` = **4**, release build, `--features
/// parallel`, `rayon`'s global pool, machine otherwise idle (see the
/// *Contention* note below, which is not a footnote — it changes the answer).
/// Ensembles of `n` independent one-equation decay problems `dy/dx = -k_i y`,
/// `y(0) = 1`, integrated from `x = 0` to `x = 1` by
/// [`Rkf45`](crate::ode::Rkf45) with `abs_tol = 1e-10`, `rel_tol = 1e-8`,
/// initial step `0.1`. Half the lanes are given `k_i` in `[0.5, 1.5)` and half
/// in `[50, 70)`, so per-lane step counts differ by more than an order of
/// magnitude and the ensemble is deliberately imbalanced. Every lane averages
/// **68.0 accepted steps** and about **8.6 us** of serial work. Best of 7
/// samples per point, wall clock for one whole ensemble. Produced by the
/// `#[ignore]`d `ensemble_crossover_benchmark` test in `parallel/tests.rs` and
/// transcribed from its printed output. Three independent runs are carried
/// side by side rather than averaged, because the parallel column is far
/// noisier than the serial one and the spread is the finding.
///
/// | Lanes | serial (run A) | speed-up A | speed-up B | speed-up C |
/// |---|---|---|---|---|
/// | 8 | 66.85 us | 2.66x | 1.26x | 0.89x |
/// | 16 | 132.56 us | 2.99x | 2.10x | 3.12x |
/// | 32 | 267.99 us | 1.23x | 1.53x | 2.51x |
/// | 64 | 545.25 us | 3.07x | 1.94x | 2.37x |
/// | 128 | 1092.75 us | 3.27x | 1.96x | 3.78x |
/// | 256 | 2212.57 us | 3.67x | 2.41x | 2.99x |
/// | 1 024 | 8814.43 us | 3.55x | 2.94x | 3.72x |
/// | 4 096 | 35205.42 us | 3.67x | 3.87x | 3.60x |
/// | 16 384 | 141229.69 us | 3.86x | 3.89x | 3.74x |
///
/// *Result.* **16** is the smallest size at which the parallel path won in all
/// three runs *and* kept winning at every larger size in all three, and it is
/// the value this constant takes. At 8 lanes it lost run C (0.89x).
///
/// *How firm is that.* Not very, and the table says so honestly. Between 16 and
/// 128 lanes the **sign** of the effect is consistent — the parallel path never
/// loses — but the **magnitude** is not resolved: 32 lanes gave 1.23x and 2.51x
/// in two runs of the same code on the same data. Anywhere in 16–128 would be
/// defensible on this evidence. What the table does establish firmly is the
/// plateau: from 1 024 lanes upward the speed-up sits at 3.5–3.9x on 4 logical
/// cores, run to run, which is close to the ideal and is the expected signature
/// of a compute-bound kernel with no cross-lane traffic.
///
/// *Interpretation.* This is the lowest crossover measured anywhere in the crate
/// — 16x below [`crate::math::parallel::ROOT_BATCH_MIN_PROBLEMS`] (256), 256x
/// below the crate-wide placeholder
/// [`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`] (4 096) and 8 192x below
/// [`crate::fields::parallel::FIELD_PARALLEL_CROSSOVER`] (131 072). The reason
/// is structural: one lane here is 68 adaptive steps, each of them six
/// derivative evaluations, against a state vector of one `f64`. At about 8.6 us
/// per lane it is by a wide margin the most compute-dense per work item of the
/// crate's measured kernels, so `rayon`'s dispatch cost is amortised almost
/// immediately. Five kernel families have now been measured in this crate and
/// they want **16, 256, 4 096, 131 072 and 262 144** — a spread of 16 384x,
/// which is the strongest evidence yet that no single crate-wide threshold can
/// be right.
///
/// The corollary is the one the root finder also found, sharpened here:
/// **the crossover is set by the caller's problem, not by this module.** A lane
/// integrating a short interval, or a two-equation system to a loose tolerance,
/// is an order of magnitude cheaper and crosses over correspondingly later. A
/// caller whose lanes are very cheap should pass [`ComputeBackend::Serial`]
/// explicitly rather than trust this number.
///
/// # Contention — this threshold assumes the cores are actually free
///
/// Three earlier runs of the same benchmark were taken while an unrelated
/// `rustc` was using ~234% CPU on the same 4-core machine (load average 5.08),
/// and they are **not** the table above. Under that load the parallel path lost
/// at 8 lanes in every run and lost once at 64 lanes, and the plateau speed-up
/// fell from 3.5–3.9x to 2.9–3.9x. The lesson generalises beyond this
/// measurement: a threshold measured on an idle machine is optimistic for a
/// process sharing cores with anything else — an MPI rank per core, a coupled
/// solver threading elsewhere, or a CI box running several jobs. In those
/// settings prefer an explicit [`ComputeBackend`].
///
/// # Limitations
///
/// One machine, four logical cores, one system family (scalar linear decay),
/// one stepper (`Rkf45`). Not measured on Android/Termux hardware, not on a
/// many-core server, and not with [`Rosenbrock23`](crate::ode::Rosenbrock23),
/// whose per-lane cost includes an LU factorisation and is therefore higher
/// still — meaning this floor is conservative for stiff ensembles rather than
/// wrong for them.
///
/// # Units
///
/// A count of independent initial-value problems, dimensionless.
pub const ODE_ENSEMBLE_MIN_LANES: usize = 16;

/// Interval count below which a [`ComputeBackend::CpuMulti`] request runs
/// [`quadrature_batch`] and [`adaptive_quadrature_batch`] on the calling thread
/// instead.
///
/// # Measured crossover
///
/// *Methodology.* Same machine, build and conditions as
/// [`ODE_ENSEMBLE_MIN_LANES`] (4 logical cores, release, `--features parallel`,
/// idle, best of 7 samples, three independent runs), measured 2026-08-13.
/// Batches of `n` independent integrals of `exp(-a_i x) sin(b_i x)` with
/// `a_i` in `[0.1, 3)` and `b_i` in `[1, 30)`, over per-lane intervals inside
/// `[0, 4]`, evaluated with [`QuadratureRule::GaussLegendre`] at
/// [`GaussOrder::G5`] over 16 panels — 80 integrand evaluations per lane, about
/// **1.4 us** of serial work, a mid-cost rule. Produced by the `#[ignore]`d
/// `quadrature_crossover_benchmark` test in `parallel/tests.rs` and transcribed
/// from its printed output.
///
/// | Intervals | serial (run A) | speed-up A | speed-up B | speed-up C |
/// |---|---|---|---|---|
/// | 16 | 20.48 us | 1.36x | 1.16x | 0.51x |
/// | 32 | 40.96 us | 2.03x | 2.10x | 1.81x |
/// | 64 | 82.62 us | 1.95x | 2.47x | 2.62x |
/// | 128 | 170.22 us | 1.96x | 1.58x | 2.75x |
/// | 256 | 351.75 us | 3.15x | 3.47x | 3.29x |
/// | 1 024 | 1475.98 us | 3.61x | 3.79x | 3.74x |
/// | 4 096 | 6090.40 us | 3.89x | 3.75x | 3.97x |
/// | 16 384 | 24717.71 us | 3.89x | 3.75x | 3.90x |
///
/// *Result.* **32** is the smallest size at which the parallel path won in all
/// three runs and kept winning at every larger size, and it is the value this
/// constant takes. At 16 intervals it lost run C badly (0.51x).
///
/// *Interpretation.* Twice the ODE floor of 16, on lanes about 6x cheaper
/// (1.4 us against 8.6 us). The two do not scale together, which is the honest
/// reading of a measurement whose 16–128 region is dominated by run-to-run
/// noise in both kernels; what both agree on is that a batch of a few hundred
/// compute-dense lanes is comfortably worth threading and a batch of a handful
/// is not. From 256 intervals upward the speed-up is a stable 3.1–4.0x on 4
/// logical cores.
///
/// **This one floor is shared by both quadrature entry points**, and it was
/// measured on the fixed-rule path only. [`adaptive_quadrature_batch`] costs
/// more per lane than any fixed rule a caller is likely to choose — the
/// verification lanes on that function needed 469 to 1 225 evaluations against
/// this rule's 80 — so it crosses over *earlier* and inherits this floor as a
/// conservative assumption rather than a measurement. A caller whose fixed rule
/// is very cheap — say [`QuadratureRule::Trapezoid`] with one panel, two
/// evaluations — is well below what was measured and should pass a backend
/// explicitly.
///
/// # Contention
///
/// The same caveat as [`ODE_ENSEMBLE_MIN_LANES`], and it bit harder here.
/// Three earlier runs taken while an unrelated `rustc` held ~234% CPU on the
/// same 4-core machine showed the parallel path **losing at every size up to
/// 64** and sitting at 0.98–1.02x — no speed-up at all — at 256 through 4 096,
/// where the idle machine gives 3.1–4.0x. A threshold measured on an idle
/// machine is optimistic for a process that shares its cores.
///
/// # Limitations
///
/// One machine, four logical cores, idle, one integrand family, one rule
/// (`G5` over 16 panels). Not measured on Android/Termux. Not measured for the
/// adaptive path, for the trapezoid or Simpson rules, or for other Gauss orders.
///
/// # Units
///
/// A count of independent definite integrals, dimensionless.
pub const QUADRATURE_MIN_INTERVALS: usize = 32;

/// Hard ceiling on adaptive bisection depth in
/// [`adaptive_quadrature_batch`], regardless of
/// [`AdaptiveSettings::max_subdivisions`].
///
/// A depth of 50 halvings shrinks an interval by a factor of `2^50` (about
/// `10^15`), at which point the sub-interval is at the rounding floor of any
/// abscissa a caller is likely to pass and further subdivision cannot improve
/// the answer. The ceiling exists so that a pathological integrand — one with a
/// genuine singularity inside the interval — terminates with
/// [`QuadratureStatus::ToleranceNotMet`] instead of consuming memory
/// indefinitely.
///
/// # Units
///
/// A count of bisections, dimensionless.
pub const MAX_ADAPTIVE_DEPTH: u32 = 50;

// ── Backend dispatch ─────────────────────────────────────────────────────────

/// Resolve a requested backend to the one this module will actually run, given
/// how much work there is.
///
/// Three reductions, in order: [`ComputeBackend::resolve`] degrades anything
/// whose feature is off; `Gpu` degrades again because this module has no GPU
/// kernel; and `CpuMulti` degrades to `Serial` below `min_work_items`. The
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

/// The [`ComputeBackend`] the ODE ensemble would actually use for `n` lanes if
/// asked for `requested` — without running anything.
///
/// Applies exactly the same reduction the kernels do (feature availability, no
/// GPU kernel here, and the [`ODE_ENSEMBLE_MIN_LANES`] size floor), so what it
/// reports is what would run. Useful for logging and for benchmark harnesses.
///
/// # Arguments
///
/// - `requested` — the backend a caller would pass to [`integrate_ensemble`].
/// - `n` — the number of independent lanes, dimensionless.
///
/// # Returns
///
/// Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`]; never
/// [`ComputeBackend::Gpu`], because no GPU kernel exists here.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::ode::parallel::{ensemble_backend_for, ODE_ENSEMBLE_MIN_LANES};
///
/// // Too small to thread, whatever was asked for.
/// assert_eq!(ensemble_backend_for(ComputeBackend::CpuMulti, 4), ComputeBackend::Serial);
///
/// // Big enough; the answer now depends only on whether `parallel` is compiled in.
/// let picked = ensemble_backend_for(ComputeBackend::CpuMulti, ODE_ENSEMBLE_MIN_LANES);
/// assert!(picked.is_available());
/// ```
#[must_use]
pub fn ensemble_backend_for(requested: ComputeBackend, n: usize) -> ComputeBackend {
    effective_backend(requested, n, ODE_ENSEMBLE_MIN_LANES)
}

/// The [`ComputeBackend`] the quadrature kernels would actually use for `n`
/// intervals if asked for `requested` — without running anything.
///
/// The quadrature counterpart of [`ensemble_backend_for`], differing only in
/// using the [`QUADRATURE_MIN_INTERVALS`] size floor. Both
/// [`quadrature_batch`] and [`adaptive_quadrature_batch`] use it.
///
/// # Arguments
///
/// - `requested` — the backend a caller would pass to [`quadrature_batch`].
/// - `n` — the number of independent definite integrals, dimensionless.
///
/// # Returns
///
/// Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`].
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::ode::parallel::quadrature_backend_for;
///
/// assert_eq!(quadrature_backend_for(ComputeBackend::CpuMulti, 8), ComputeBackend::Serial);
/// assert_eq!(quadrature_backend_for(ComputeBackend::Serial, 1 << 20), ComputeBackend::Serial);
/// ```
#[must_use]
pub fn quadrature_backend_for(requested: ComputeBackend, n: usize) -> ComputeBackend {
    effective_backend(requested, n, QUADRATURE_MIN_INTERVALS)
}

// ═════════════════════════════════════════════════════════════════════════════
// Part A — ODE ensembles
// ═════════════════════════════════════════════════════════════════════════════

/// One member of an ODE ensemble: a system, its initial condition, and the
/// interval to advance it over.
///
/// The system is owned **by value**, so an ensemble is a plain
/// `Vec<OdeLane<S>>` with no lifetime parameter anywhere and no possibility of
/// the system slice and the initial-condition slice disagreeing in length. `S`
/// may be any concrete type implementing [`OdeSystem`], including the caller's
/// own enum over several systems — which is how one ensemble holds genuinely
/// different physics in different lanes.
///
/// # Units
///
/// `x_start`, `x_end` and `dx0` are in the independent variable's units
/// (typically seconds); `y0`'s components are in whatever units that lane's
/// state carries, which need not be the same as each other. All are the
/// caller's own units — see the module-level "Units" section.
///
/// # Validity
///
/// A lane is rejected with [`OdeLaneStatus::InvalidLane`], before any
/// integration, unless all of the following hold:
///
/// - `y0.len() == system.n_eqns()`
/// - `x_start`, `x_end` and every component of `y0` are finite
/// - `dx0` is finite and strictly positive
/// - `x_end >= x_start` — **the underlying interval loop integrates forwards
///   only**, so a reversed interval is a caller error rather than a backwards
///   integration
///
/// `x_end == x_start` is legal and is a no-op: the lane completes in zero steps
/// with its state unchanged.
#[derive(Debug, Clone, PartialEq)]
pub struct OdeLane<S: OdeSystem> {
    /// The system this lane integrates, owned outright.
    pub system: S,
    /// Initial state at `x_start`, one entry per equation.
    pub y0: Vec<f64>,
    /// Start of the integration interval.
    pub x_start: f64,
    /// End of the integration interval; must be `>= x_start`.
    pub x_end: f64,
    /// First step size to attempt. Must be finite and `> 0`; the adaptive
    /// controller adjusts it from there, so it is a starting guess and not a
    /// constraint.
    pub dx0: f64,
}

impl<S: OdeSystem> OdeLane<S> {
    /// Build one ensemble lane.
    ///
    /// # Arguments
    ///
    /// - `system` — the system to integrate, moved into the lane.
    /// - `y0` — initial state, `system.n_eqns()` entries.
    /// - `x_start`, `x_end` — the interval, in the independent variable's units,
    ///   with `x_end >= x_start`.
    /// - `dx0` — first step size to attempt, same units, `> 0`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::ode::OdeSystem;
    /// use outram_foam_basic_lib::ode::parallel::OdeLane;
    ///
    /// struct Decay;
    /// impl OdeSystem for Decay {
    ///     fn n_eqns(&self) -> usize { 1 }
    ///     fn derivatives(&self, _x: f64, y: &[f64], d: &mut Vec<f64>) { d[0] = -y[0]; }
    /// }
    ///
    /// let lane = OdeLane::new(Decay, vec![1.0], 0.0, 1.0, 0.1);
    /// assert_eq!(lane.y0.len(), 1);
    /// ```
    pub fn new(system: S, y0: Vec<f64>, x_start: f64, x_end: f64, dx0: f64) -> Self {
        Self {
            system,
            y0,
            x_start,
            x_end,
            dx0,
        }
    }
}

/// How one lane of an ODE ensemble ended.
///
/// Only [`Completed`](Self::Completed) means the lane reached `x_end`. Every
/// other variant is a failure a caller must handle; see the module-level
/// "Non-convergence is reported, never swallowed" section for the accessors that
/// make it hard to skip.
///
/// | Variant | `last_state` | Meaning |
/// |---|---|---|
/// | [`Completed`](Self::Completed) | the state at `x_end` | reached the end of the interval |
/// | [`MaxStepsExceeded`](Self::MaxStepsExceeded) | genuine partial state at `x_reached` — **not** the answer | ran out of sub-steps |
/// | [`StepSizeUnderflow`](Self::StepSizeUnderflow) | genuine partial state at `x_reached` | the step shrank below `f64::EPSILON` |
/// | [`NotFinite`](Self::NotFinite) | the non-finite state | the state went `NaN`/infinite |
/// | [`InvalidLane`](Self::InvalidLane) | all `NaN` | the lane's inputs were unusable |
///
/// The two middle variants are the *stiffness signature*: an explicit stepper on
/// a stiff lane runs out of budget rather than returning a wrong answer.
///
/// # Units
///
/// Dimensionless — a status tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OdeLaneStatus {
    /// The lane reached `x_end` with a finite state.
    Completed,
    /// `OdeSolverConfig::max_steps` sub-steps were taken without spanning the
    /// interval. The reported state is genuine but partial, at
    /// [`OdeLaneSolution::x_reached`].
    MaxStepsExceeded,
    /// The adaptive controller shrank the step below `f64::EPSILON` trying to
    /// meet the tolerance — the system is too stiff for the chosen stepper, or
    /// the tolerances are unattainable. Maps to
    /// [`OdeError::StepSizeUnderflow`](crate::ode::OdeError::StepSizeUnderflow).
    StepSizeUnderflow,
    /// The integration returned success but the final state contains a `NaN` or
    /// an infinity. Reported separately from the two budget failures because it
    /// means the *model* blew up, not that the stepper ran out of room.
    NotFinite,
    /// The lane was rejected before any integration — see [`OdeLane`]'s
    /// "Validity" section for the exact conditions. The state is all `NaN`,
    /// because returning the untouched initial condition would look like a
    /// zero-length integration that succeeded.
    InvalidLane,
}

impl OdeLaneStatus {
    /// Whether this status means the lane produced a usable final state.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::ode::parallel::OdeLaneStatus;
    ///
    /// assert!(OdeLaneStatus::Completed.is_completed());
    /// assert!(!OdeLaneStatus::MaxStepsExceeded.is_completed());
    /// ```
    #[must_use]
    pub fn is_completed(self) -> bool {
        matches!(self, Self::Completed)
    }

    /// A short human-readable label, for log lines and failure reports.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::MaxStepsExceeded => "max-steps-exceeded",
            Self::StepSizeUnderflow => "step-size-underflow",
            Self::NotFinite => "not-finite",
            Self::InvalidLane => "invalid-lane",
        }
    }
}

/// The outcome of a single ensemble lane: its status, its final state, how far
/// it got and how much work it took.
///
/// The fields are private on purpose. The only way to get a state that is
/// claimed to be the answer is [`Self::state`], which returns `Option<&[f64]>`
/// and hands back `Some` only for a completed lane. The raw state is available
/// from [`Self::last_state`], whose name is chosen so that using a failed lane's
/// partial trajectory is a visible decision in the calling code.
///
/// # Units
///
/// [`Self::state`] and [`Self::last_state`] carry the lane's own state units;
/// [`Self::x_reached`] and [`Self::dx_final`] are in the independent variable's
/// units; [`Self::steps`] is a dimensionless count.
#[derive(Debug, Clone, PartialEq)]
pub struct OdeLaneSolution {
    y: Vec<f64>,
    x_reached: f64,
    dx_final: f64,
    steps: u32,
    status: OdeLaneStatus,
}

impl OdeLaneSolution {
    /// The final state, if this lane reached `x_end`.
    ///
    /// `None` for every failure status. This is the accessor to reach for; the
    /// `Option` is the point.
    #[must_use]
    pub fn state(&self) -> Option<&[f64]> {
        if self.status.is_completed() {
            Some(&self.y)
        } else {
            None
        }
    }

    /// The last state the stepper held, completed or not.
    ///
    /// **The answer only when [`Self::status`] is
    /// [`OdeLaneStatus::Completed`].** For
    /// [`OdeLaneStatus::MaxStepsExceeded`] and
    /// [`OdeLaneStatus::StepSizeUnderflow`] this is a genuine state on the
    /// trajectory — the state at [`Self::x_reached`], not at `x_end` — and is
    /// offered so the caller can restart, sub-cycle, or re-run the lane with a
    /// stiff stepper. For [`OdeLaneStatus::InvalidLane`] every component is
    /// `NaN`, because there is no honest number to report.
    #[must_use]
    pub fn last_state(&self) -> &[f64] {
        &self.y
    }

    /// The independent-variable value the trajectory actually reached.
    ///
    /// Equal to the last accepted step's endpoint, which for a completed lane is
    /// `x_end` up to the accumulated rounding of the step sequence — it is the
    /// measured value, not `x_end` substituted in. `NaN` for
    /// [`OdeLaneStatus::InvalidLane`].
    #[must_use]
    pub fn x_reached(&self) -> f64 {
        self.x_reached
    }

    /// The step size the adaptive controller ended on, in the independent
    /// variable's units.
    ///
    /// Worth carrying forward as the next call's `dx0` when marching an ensemble
    /// through many intervals: it saves the controller re-discovering the scale
    /// every interval. `NaN` for [`OdeLaneStatus::InvalidLane`].
    #[must_use]
    pub fn dx_final(&self) -> f64 {
        self.dx_final
    }

    /// Accepted sub-steps this lane took, dimensionless.
    ///
    /// Counts *accepted* steps only; a step rejected by the error controller and
    /// retried at a smaller size is not counted separately, because the retry
    /// loop lives inside the stepper. This is the number to look at when
    /// diagnosing which lanes are dragging an ensemble — a lane taking 50x the
    /// median is the stiff one.
    #[must_use]
    pub fn steps(&self) -> u32 {
        self.steps
    }

    /// How this lane ended.
    #[must_use]
    pub fn status(&self) -> OdeLaneStatus {
        self.status
    }

    /// Whether this lane reached `x_end`.
    #[must_use]
    pub fn completed(&self) -> bool {
        self.status.is_completed()
    }
}

/// A batch of `N` lane outcomes, in the same order as the lanes handed in.
///
/// Lane `i` of the result corresponds to `lanes[i]`, always — the parallel path
/// preserves order, so no index bookkeeping is needed.
///
/// # Getting states out
///
/// - [`Self::states`] / [`Self::into_states`] — all-or-nothing. `Ok` only when
///   every lane completed; otherwise `Err(`[`OdeEnsembleFailure`]`)`.
/// - [`Self::lanes`] — per-lane, when the caller wants to handle failures
///   individually (re-run stiff, sub-cycle, flag the cell).
///
/// # Units
///
/// See [`OdeLaneSolution`].
#[derive(Debug, Clone, PartialEq)]
pub struct OdeEnsemble {
    lanes: Vec<OdeLaneSolution>,
}

impl OdeEnsemble {
    /// Every lane's outcome, in input order.
    #[must_use]
    pub fn lanes(&self) -> &[OdeLaneSolution] {
        &self.lanes
    }

    /// Consume the ensemble and take the outcomes.
    #[must_use]
    pub fn into_lanes(self) -> Vec<OdeLaneSolution> {
        self.lanes
    }

    /// Number of lanes, dimensionless.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lanes.len()
    }

    /// Whether the ensemble has no lanes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lanes.is_empty()
    }

    /// Lane `i`'s outcome, or `None` if `i` is out of range.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&OdeLaneSolution> {
        self.lanes.get(i)
    }

    /// Whether every lane completed. Vacuously `true` for an empty ensemble.
    #[must_use]
    pub fn all_completed(&self) -> bool {
        self.lanes.iter().all(OdeLaneSolution::completed)
    }

    /// How many lanes failed, dimensionless.
    #[must_use]
    pub fn failure_count(&self) -> usize {
        self.lanes.iter().filter(|l| !l.completed()).count()
    }

    /// The first failing lane's index and outcome, if any.
    ///
    /// The natural thing to print when an ensemble fails: it names the cell to
    /// look at rather than just the count.
    #[must_use]
    pub fn first_failure(&self) -> Option<(usize, &OdeLaneSolution)> {
        self.lanes.iter().enumerate().find(|(_, l)| !l.completed())
    }

    /// Every failing lane, as `(index, outcome)` pairs.
    ///
    /// Allocates, so prefer [`Self::first_failure`] or [`Self::failure_count`]
    /// on a hot path.
    #[must_use]
    pub fn failures(&self) -> Vec<(usize, &OdeLaneSolution)> {
        self.lanes
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.completed())
            .collect()
    }

    /// Total accepted sub-steps over every lane, dimensionless.
    ///
    /// The honest measure of how much work the ensemble did, and — read beside
    /// [`Self::max_steps_taken`] — the measure of how imbalanced it was.
    #[must_use]
    pub fn total_steps(&self) -> u64 {
        self.lanes.iter().map(|l| u64::from(l.steps)).sum()
    }

    /// The largest accepted-sub-step count over all lanes, dimensionless; `0`
    /// for an empty ensemble.
    ///
    /// With [`Self::total_steps`] this quantifies the load imbalance the
    /// module-level "Load imbalance" section describes: a maximum far above the
    /// mean is precisely the case that makes work-stealing worth having.
    #[must_use]
    pub fn max_steps_taken(&self) -> u32 {
        self.lanes.iter().map(|l| l.steps).max().unwrap_or(0)
    }

    /// Every lane's final state, or an error naming the failures — the
    /// all-or-nothing path.
    ///
    /// # Returns
    ///
    /// `Ok(v)` with `v[i]` the final state of lane `i`, in that lane's own
    /// units, when every lane completed. Otherwise
    /// `Err(`[`OdeEnsembleFailure`]`)` carrying the failure count and the first
    /// failing lane. **No `Vec` of plausible-looking states is ever returned for
    /// an ensemble that contained a failure.**
    ///
    /// An empty ensemble returns `Ok(vec![])`.
    ///
    /// # Errors
    ///
    /// [`OdeEnsembleFailure`] when one or more lanes did not complete.
    pub fn states(&self) -> Result<Vec<Vec<f64>>, OdeEnsembleFailure> {
        self.check()?;
        Ok(self.lanes.iter().map(|l| l.y.clone()).collect())
    }

    /// [`Self::states`] without the clone — consumes the ensemble.
    ///
    /// # Errors
    ///
    /// [`OdeEnsembleFailure`] when one or more lanes did not complete.
    pub fn into_states(self) -> Result<Vec<Vec<f64>>, OdeEnsembleFailure> {
        self.check()?;
        Ok(self.lanes.into_iter().map(|l| l.y).collect())
    }

    /// Shared failure check behind [`Self::states`] and [`Self::into_states`].
    fn check(&self) -> Result<(), OdeEnsembleFailure> {
        if let Some((i, l)) = self.first_failure() {
            return Err(OdeEnsembleFailure {
                total: self.lanes.len(),
                failure_count: self.failure_count(),
                first_index: i,
                first_status: l.status(),
                first_steps: l.steps(),
                first_x_reached: l.x_reached(),
            });
        }
        Ok(())
    }
}

/// One or more lanes of an [`OdeEnsemble`] did not complete.
///
/// Returned by [`OdeEnsemble::states`]. It names both the scale of the problem
/// (how many of how many) and a specific lane to look at, because "3 of 10 000
/// cells failed" is only actionable once you know *which* cell and *how far* it
/// got.
///
/// # Units
///
/// Counts and indices are dimensionless; `first_x_reached` is in the independent
/// variable's units.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[error(
    "{failure_count} of {total} ODE lanes did not complete; \
     first failure at lane {first_index} with status {first_status:?} \
     after {first_steps} steps, reaching x = {first_x_reached}"
)]
pub struct OdeEnsembleFailure {
    /// Number of lanes in the ensemble.
    pub total: usize,
    /// Number of lanes that did not complete.
    pub failure_count: usize,
    /// Index of the first failing lane.
    pub first_index: usize,
    /// Why that lane failed.
    pub first_status: OdeLaneStatus,
    /// Accepted sub-steps that lane took before giving up.
    pub first_steps: u32,
    /// The independent-variable value that lane reached.
    pub first_x_reached: f64,
}

// ── ODE ensemble entry points ────────────────────────────────────────────────

/// Integrate `N` independent initial-value problems with **one stepper for every
/// lane**, on the chosen backend.
///
/// This is the entry point for the common case: one ODE per cell, per particle,
/// or per material point, all of the same character. Each lane gets its own
/// clone of `solver`, so no scratch buffer is shared and no lane can perturb
/// another.
///
/// For an ensemble of mixed stiffness, use [`integrate_ensemble_mixed`] — see
/// the module-level "Stiffness" section.
///
/// # Arguments
///
/// - `lanes` — one [`OdeLane`] per problem. Lane `i` of the result corresponds
///   to `lanes[i]`.
/// - `solver` — the stepper prototype, cloned once per lane. Its
///   [`OdeSolverConfig`](crate::ode::OdeSolverConfig) tolerances and
///   `max_steps` apply to every lane. It must have been built for the same
///   equation count the lanes' systems report, because the steppers size their
///   scratch buffers at construction.
/// - `backend` — requested execution backend. What actually runs is
///   [`ensemble_backend_for`] applied to it: an unavailable backend degrades,
///   `Gpu` degrades (no GPU kernel here), and an ensemble below
///   [`ODE_ENSEMBLE_MIN_LANES`] runs serially. None of those changes the answer.
///
/// # Returns
///
/// An [`OdeEnsemble`] with one [`OdeLaneSolution`] per lane, in input order.
///
/// # Determinism
///
/// Bit-for-bit identical across backends and thread counts, for systems whose
/// `derivatives`/`jacobian` are pure deterministic functions of their arguments.
/// See the module-level "Determinism" section.
///
/// # Cost note
///
/// `solver` is cloned once per lane, which clones its per-equation scratch
/// buffers — a handful of small allocations per lane. This is deliberate: it is
/// what makes each lane a pure function of its own inputs, and therefore what
/// makes the bitwise-identity claim hold without depending on every stepper's
/// buffers being write-before-read. On the measured ensemble it is a small
/// fraction of the per-lane cost — about 8.6 us of integration per lane over 68
/// adaptive steps, see [`ODE_ENSEMBLE_MIN_LANES`] — but a caller integrating a
/// *very* short interval per lane would see it, and it has not been measured
/// separately from the integration it accompanies.
///
/// # Panics
///
/// Panics if `solver` is [`OdeSolver::Rosenbrock23`] and a lane's system does
/// not override
/// [`OdeSystem::jacobian`](crate::ode::OdeSystem::jacobian), whose default
/// implementation panics. On the `CpuMulti` path the panic propagates out of the
/// `rayon` scope. Check
/// [`OdeSolver::requires_jacobian`](crate::ode::OdeSolver::requires_jacobian)
/// first.
///
/// # Verification
///
/// *Methodology.* Checked against the closed-form solution of `dy/dx = -k y`,
/// `y(0) = 1`, namely `y(x) = exp(-k x)`, over 64 lanes with `k` spread evenly
/// across `[0.5, 8]`, integrated to `x = 1` by all three steppers; and against
/// the harmonic oscillator `y1' = -y2`, `y2' = y1` from `y(0) = (1, 0)`, whose
/// solution is `(cos x, sin x)`, over 16 lanes ending at `m * pi/2` for
/// `m = 1..=16`. Tolerances `abs_tol = 1e-10`, `rel_tol = 1e-8` for `Rkf45` and
/// `Rosenbrock23` (`1e-12` / `1e-10` for the oscillator) and `1e-3` / `1e-2`
/// for `Euler`, which cannot reach the high-order tolerances inside the default
/// 10 000-step budget. Pass criteria: `< 1e-8` for the high-order steppers,
/// `< 5e-2` for first-order `Euler`.
///
/// *Results, measured 2026-08-13 by `ensemble_matches_analytic_decay` and
/// `ensemble_matches_harmonic_oscillator` in `parallel/tests.rs`, release
/// build:* worst absolute error over the 64 decay lanes **1.518896e-9**
/// (`Rkf45`, 3 038 total accepted steps, 81 in the worst lane),
/// **1.387431e-9** (`Rosenbrock23`, 46 535 steps, 1 186 in the worst lane) and
/// **1.297370e-3** (`Euler`, 21 817 steps, 421 in the worst lane); worst
/// absolute error on the oscillator **4.905929e-10** (`Rkf45`).
///
/// *Interpretation.* The two high-order steppers agree with the closed form to
/// their requested tolerance and with each other to about 1e-9, while Euler is
/// six orders coarser — the expected signature of three genuinely different
/// steppers being reached through the ensemble, which is what rules out the
/// wrapper silently routing every lane to one of them. The step counts are the
/// other half of the story: `Rosenbrock23` needs 15x the steps of `Rkf45` on a
/// non-stiff problem to reach the same accuracy, which is exactly why
/// [`integrate_ensemble_mixed`] exists rather than "just use the stiff solver
/// everywhere".
///
/// # Example — the `uom` boundary
///
/// The ensemble is dimensionless, and the caller converts at its edge. These
/// lanes are lumped-capacitance bodies cooling towards ambient, `dT/dt =
/// -(T - T_inf) / tau`, one lane per time constant, recovering a
/// `ThermodynamicTemperature`.
///
/// **On the tolerance asserted below.** The stepper's `rel_tol` is `1e-8` and
/// the states are of order 400 K, so the achievable *absolute* accuracy is a
/// few microkelvin — the error floor is set by the controller's tolerance and
/// the magnitude of the state, not by the ensemble. Measured 2026-08-13
/// (release) by `lumped_body_accuracy_is_set_by_the_relative_tolerance` in
/// `parallel/tests.rs`: worst absolute error **1.150937e-6 K** at
/// `abs_tol = 1e-10`, `rel_tol = 1e-8`; **1.222958e-8 K** at `1e-12`/`1e-10`;
/// **1.289209e-10 K** at `1e-14`/`1e-12`. The bound below is `5e-6 K`, set from
/// the first of those measurements — a tighter assertion would be asserting
/// something this stepper at this tolerance does not deliver. A caller wanting
/// sub-nanokelvin must ask for it through the tolerances, and the third row
/// says what that buys.
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::ode::{OdeSolver, OdeSystem};
/// use outram_foam_basic_lib::ode::parallel::{integrate_ensemble, OdeLane};
/// use uom::si::f64::{ThermodynamicTemperature, Time};
/// use uom::si::thermodynamic_temperature::kelvin;
/// use uom::si::time::second;
///
/// /// Lumped body: `dT/dt = -(T - T_inf) / tau`, all in SI base units.
/// struct LumpedBody { tau_s: f64, t_inf_k: f64 }
/// impl OdeSystem for LumpedBody {
///     fn n_eqns(&self) -> usize { 1 }
///     fn derivatives(&self, _t: f64, y: &[f64], dydt: &mut Vec<f64>) {
///         dydt[0] = -(y[0] - self.t_inf_k) / self.tau_s;
///     }
/// }
///
/// // Convert in: typed quantities out to plain f64 in named units.
/// let t_inf = ThermodynamicTemperature::new::<kelvin>(300.0);
/// let t0 = ThermodynamicTemperature::new::<kelvin>(500.0);
/// let horizon = Time::new::<second>(10.0);
///
/// let lanes: Vec<OdeLane<LumpedBody>> = [5.0_f64, 20.0]
///     .iter()
///     .map(|&tau| {
///         OdeLane::new(
///             LumpedBody { tau_s: tau, t_inf_k: t_inf.get::<kelvin>() },
///             vec![t0.get::<kelvin>()],
///             0.0,
///             horizon.get::<second>(),
///             0.1,
///         )
///     })
///     .collect();
///
/// let ensemble = integrate_ensemble(
///     &lanes,
///     &OdeSolver::rkf45(1, 1e-10, 1e-8),
///     ComputeBackend::CpuMulti,
/// );
///
/// // Convert out: back to typed temperatures.
/// let temperatures: Vec<ThermodynamicTemperature> = ensemble
///     .states()
///     .expect("both lanes complete")
///     .iter()
///     .map(|s| ThermodynamicTemperature::new::<kelvin>(s[0]))
///     .collect();
///
/// // Closed form: T(t) = T_inf + (T0 - T_inf) exp(-t / tau). The 5e-6 K bound
/// // is the measured floor at rel_tol = 1e-8 on a ~400 K state; see above.
/// for (tau, temperature) in [5.0_f64, 20.0].iter().zip(&temperatures) {
///     let exact = 300.0 + 200.0 * (-10.0_f64 / tau).exp();
///     assert!((temperature.get::<kelvin>() - exact).abs() < 5e-6);
/// }
/// ```
#[must_use]
pub fn integrate_ensemble<S>(
    lanes: &[OdeLane<S>],
    solver: &OdeSolver,
    backend: ComputeBackend,
) -> OdeEnsemble
where
    S: OdeSystem + Sync,
{
    integrate_ensemble_mixed(lanes, |_| solver.clone(), backend)
}

/// Integrate `N` independent initial-value problems with the **stepper chosen
/// per lane**, on the chosen backend.
///
/// The mixed-stiffness entry point. `solver_of(i)` is called once for lane `i`
/// and its return value integrates that lane and no other, so a caller can put
/// [`Rosenbrock23`](crate::ode::Rosenbrock23) on the handful of stiff cells and
/// leave the rest on [`Rkf45`](crate::ode::Rkf45) — paying the LU factorisation
/// only where it is needed.
///
/// The natural way to use it is as a second pass: run
/// [`integrate_ensemble`] with an explicit stepper, read
/// [`OdeEnsemble::failures`], and re-run just those lanes stiff.
///
/// # Arguments
///
/// - `lanes` — one [`OdeLane`] per problem.
/// - `solver_of` — `solver_of(i)` returns the stepper for lane `i`. Called
///   exactly once per lane, on whichever thread runs that lane, hence the `Sync`
///   bound; the bound is present in both feature builds so that enabling
///   `parallel` never changes a public signature. It must be a pure
///   deterministic function of `i` — see the module-level "Determinism" section.
/// - `backend` — requested backend; see [`ensemble_backend_for`].
///
/// # Returns
///
/// An [`OdeEnsemble`] with one [`OdeLaneSolution`] per lane, in input order.
///
/// # Panics
///
/// As [`integrate_ensemble`]: a lane given
/// [`OdeSolver::Rosenbrock23`] whose system does not implement
/// [`OdeSystem::jacobian`](crate::ode::OdeSystem::jacobian) panics.
///
/// # Example — a stiff lane and a benign one in one ensemble
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::matrix::SquareMatrix;
/// use outram_foam_basic_lib::ode::{OdeSolver, OdeSystem};
/// use outram_foam_basic_lib::ode::parallel::{integrate_ensemble_mixed, OdeLane};
///
/// /// `dy/dx = -k y`, with an analytic Jacobian so the stiff stepper can run.
/// struct Decay { k: f64 }
/// impl OdeSystem for Decay {
///     fn n_eqns(&self) -> usize { 1 }
///     fn derivatives(&self, _x: f64, y: &[f64], d: &mut Vec<f64>) { d[0] = -self.k * y[0]; }
///     fn jacobian(&self, _x: f64, _y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
///         dfdx[0] = 0.0;
///         dfdy.set(0, 0, -self.k);
///     }
/// }
///
/// // Lane 0 is benign (k = 1), lane 1 is stiff (k = 5000).
/// let lanes = vec![
///     OdeLane::new(Decay { k: 1.0 }, vec![1.0], 0.0, 1.0, 0.1),
///     OdeLane::new(Decay { k: 5000.0 }, vec![1.0], 0.0, 1.0, 0.1),
/// ];
///
/// let ensemble = integrate_ensemble_mixed(
///     &lanes,
///     |i| {
///         if i == 1 {
///             OdeSolver::rosenbrock23(1, 1e-10, 1e-8)
///         } else {
///             OdeSolver::rkf45(1, 1e-10, 1e-8)
///         }
///     },
///     ComputeBackend::Serial,
/// );
///
/// assert!(ensemble.all_completed());
/// let states = ensemble.states().unwrap();
/// assert!((states[0][0] - (-1.0_f64).exp()).abs() < 1e-8);
/// assert!(states[1][0].abs() < 1e-8); // exp(-5000) underflows to ~0
/// ```
#[must_use]
pub fn integrate_ensemble_mixed<S, G>(
    lanes: &[OdeLane<S>],
    solver_of: G,
    backend: ComputeBackend,
) -> OdeEnsemble
where
    S: OdeSystem + Sync,
    G: Fn(usize) -> OdeSolver + Sync,
{
    integrate_ensemble_min(lanes, solver_of, backend, ODE_ENSEMBLE_MIN_LANES)
}

/// [`integrate_ensemble_mixed`] with the size floor supplied by the caller.
///
/// Exists so the crossover benchmark can measure the multi-CPU path *below*
/// [`ODE_ENSEMBLE_MIN_LANES`] — the only way to find where the crossover
/// actually is — and so the cross-backend bitwise tests are not vacuous on small
/// ensembles. Not public: production callers get the measured floor.
pub(crate) fn integrate_ensemble_min<S, G>(
    lanes: &[OdeLane<S>],
    solver_of: G,
    backend: ComputeBackend,
    min_lanes: usize,
) -> OdeEnsemble
where
    S: OdeSystem + Sync,
    G: Fn(usize) -> OdeSolver + Sync,
{
    let n = lanes.len();
    let solutions: Vec<OdeLaneSolution> = match effective_backend(backend, n, min_lanes) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => lanes
            .par_iter()
            .enumerate()
            // No `min_len` floor: adaptive step counts vary by orders of
            // magnitude between lanes, so the splitter is left free to steal
            // down to a single lane.
            .map(|(i, lane)| integrate_one_lane(lane, &mut solver_of(i)))
            .collect(),
        _ => lanes
            .iter()
            .enumerate()
            .map(|(i, lane)| integrate_one_lane(lane, &mut solver_of(i)))
            .collect(),
    };
    OdeEnsemble { lanes: solutions }
}

/// Integrate one lane — the single per-lane kernel both backends call.
///
/// Drives the crate's existing [`OdeSolver::solve_step`] through the crate's
/// existing `integrate_interval` loop, wrapping the step closure only to count
/// accepted steps and record how far the trajectory actually got. No integration
/// arithmetic is duplicated here.
#[inline]
fn integrate_one_lane<S>(lane: &OdeLane<S>, solver: &mut OdeSolver) -> OdeLaneSolution
where
    S: OdeSystem,
{
    let n_eqns = lane.system.n_eqns();
    let valid = lane.y0.len() == n_eqns
        && lane.x_start.is_finite()
        && lane.x_end.is_finite()
        && lane.dx0.is_finite()
        && lane.dx0 > 0.0
        && lane.x_end >= lane.x_start
        && lane.y0.iter().all(|v| v.is_finite());

    if !valid {
        return OdeLaneSolution {
            y: vec![f64::NAN; lane.y0.len()],
            x_reached: f64::NAN,
            dx_final: f64::NAN,
            steps: 0,
            status: OdeLaneStatus::InvalidLane,
        };
    }

    let mut y = lane.y0.clone();
    let mut dx = lane.dx0;

    // A zero-length interval is a legitimate no-op; `integrate_interval`'s
    // `while x < x_end` would do nothing anyway, but saying so explicitly keeps
    // the reported `x_reached` honest.
    if lane.x_end == lane.x_start {
        return OdeLaneSolution {
            y,
            x_reached: lane.x_start,
            dx_final: dx,
            steps: 0,
            status: OdeLaneStatus::Completed,
        };
    }

    let cfg = solver.config().clone();
    let system = &lane.system;
    let mut steps: u32 = 0;
    let mut x_reached = lane.x_start;

    let outcome = super::integrate_interval(
        &cfg,
        &mut |x, y, dx| {
            let result = solver.solve_step(system, x, y, dx);
            if result.is_ok() {
                steps = steps.saturating_add(1);
                x_reached = *x;
            }
            result
        },
        lane.x_start,
        lane.x_end,
        &mut y,
        &mut dx,
    );

    let status = match outcome {
        Ok(()) if y.iter().all(|v| v.is_finite()) => OdeLaneStatus::Completed,
        Ok(()) => OdeLaneStatus::NotFinite,
        Err(OdeError::StepSizeUnderflow) => OdeLaneStatus::StepSizeUnderflow,
        Err(OdeError::MaxStepsExceeded(_)) => OdeLaneStatus::MaxStepsExceeded,
    };

    OdeLaneSolution {
        y,
        x_reached,
        dx_final: dx,
        steps,
        status,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Part B — batched quadrature
// ═════════════════════════════════════════════════════════════════════════════

/// One lane of a quadrature batch: the limits of one definite integral.
///
/// # Units
///
/// `a` and `b` are in the integration variable's units. The value the batch
/// returns carries the product of those units and the integrand's.
///
/// # Conventions
///
/// - `a == b` integrates to exactly `0.0`, with no integrand evaluations.
/// - `b < a` is **supported** and returns the negated integral over `[b, a]`,
///   the usual orientation convention. (The ODE half deliberately does *not*
///   accept a reversed interval, because its underlying loop marches forwards
///   only; quadrature has no such constraint.)
/// - A non-finite `a` or `b` yields [`QuadratureStatus::InvalidInterval`]. There
///   is no support for infinite limits; a caller wanting one should substitute
///   the variable itself, which is the only way to choose the transformation
///   knowingly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuadratureInterval {
    /// Lower limit, in the integration variable's units.
    pub a: f64,
    /// Upper limit, in the integration variable's units.
    pub b: f64,
}

impl QuadratureInterval {
    /// Build an interval `[a, b]`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::ode::parallel::QuadratureInterval;
    ///
    /// let iv = QuadratureInterval::new(0.0, 1.0);
    /// assert_eq!(iv.b - iv.a, 1.0);
    /// ```
    #[must_use]
    pub fn new(a: f64, b: f64) -> Self {
        Self { a, b }
    }
}

/// Node count of a fixed-order Gauss-Legendre rule.
///
/// A closed set rather than an open `usize`, so the choice is exhaustive at
/// every match site and rust-analyzer lists the options on hover — and so the
/// nodes can be validated once per order rather than for whatever number a
/// caller happens to pass.
///
/// An `n`-point Gauss-Legendre rule integrates any polynomial of degree
/// `2n - 1` or less **exactly** (to rounding), which is both the reason to
/// prefer it over Simpson at equal cost and the property the tests use as their
/// oracle.
///
/// | Order | Nodes | Exact to degree |
/// |---|---|---|
/// | [`G2`](Self::G2) | 2 | 3 |
/// | [`G3`](Self::G3) | 3 | 5 |
/// | [`G4`](Self::G4) | 4 | 7 |
/// | [`G5`](Self::G5) | 5 | 9 |
/// | [`G8`](Self::G8) | 8 | 15 |
///
/// # Units
///
/// Dimensionless — a mode selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GaussOrder {
    /// Two-point rule, exact to cubics.
    G2,
    /// Three-point rule, exact to quintics.
    G3,
    /// Four-point rule, exact to degree 7.
    #[default]
    G4,
    /// Five-point rule, exact to degree 9.
    G5,
    /// Eight-point rule, exact to degree 15.
    G8,
}

impl GaussOrder {
    /// Number of nodes in the rule, dimensionless.
    #[must_use]
    pub const fn points(self) -> usize {
        match self {
            Self::G2 => 2,
            Self::G3 => 3,
            Self::G4 => 4,
            Self::G5 => 5,
            Self::G8 => 8,
        }
    }

    /// The highest polynomial degree this rule integrates exactly, `2n - 1`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::ode::parallel::GaussOrder;
    ///
    /// assert_eq!(GaussOrder::G5.points(), 5);
    /// assert_eq!(GaussOrder::G5.exact_degree(), 9);
    /// ```
    #[must_use]
    pub const fn exact_degree(self) -> usize {
        2 * self.points() - 1
    }
}

/// Which fixed rule [`quadrature_batch`] applies to every lane.
///
/// All three are **composite** rules over equal subintervals of each lane's own
/// interval, and all three cost a fixed, data-independent number of integrand
/// evaluations per lane — which is what makes them branch-free, and the only
/// part of this module that would map cleanly onto a GPU.
///
/// A `panels` count of `0` is treated as `1`; there is no error path for it,
/// because it is a compile-time-shaped parameter rather than data.
///
/// | Variant | Evaluations per lane | Error order | Exact for |
/// |---|---|---|---|
/// | [`Trapezoid`](Self::Trapezoid) | `panels + 1` | `O(h^2)` | linear integrands |
/// | [`Simpson`](Self::Simpson) | `2 * panels + 1` | `O(h^4)` | cubics |
/// | [`GaussLegendre`](Self::GaussLegendre) | `panels * order` | `O(h^(2n))` | degree `2n - 1` |
///
/// # Relationship to the workspace's other Simpson
///
/// `outram-park-fork-dwsim-libs`'s `simpson_integrate` is a faithful port of
/// OPEM's rule *including* its documented flaw: it applies the `1, 4, 2, ..., 4,
/// 1` weights without requiring an odd sample count, which silently degrades the
/// order to `O(h)` for an even one. [`Simpson`](Self::Simpson) here counts
/// **panels of two subintervals each**, so the sample count is odd by
/// construction and that failure cannot occur. The composite-over-equal-
/// subintervals convention is shared; the sample-slice interface and the bug are
/// not.
///
/// # Units
///
/// Dimensionless — a mode selector plus a count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuadratureRule {
    /// Composite trapezoid over `panels` equal subintervals.
    ///
    /// The cheapest rule and the only one worth using when the integrand is
    /// sampled rather than smooth. Second-order accurate.
    Trapezoid {
        /// Number of equal subintervals, `>= 1` (`0` is treated as `1`).
        panels: usize,
    },
    /// Composite Simpson over `panels` panels, each spanning **two** equal
    /// subintervals, so `2 * panels` subintervals in total.
    ///
    /// Fourth-order accurate and exact for cubics. Counting panels rather than
    /// samples is what makes the even-sample-count error impossible.
    Simpson {
        /// Number of two-subinterval Simpson panels, `>= 1` (`0` is treated as
        /// `1`).
        panels: usize,
    },
    /// Composite Gauss-Legendre: `panels` equal subintervals, each integrated by
    /// an `order`-point rule.
    ///
    /// The best accuracy per evaluation for a smooth integrand, and the rule to
    /// reach for by default. Gauss rules never evaluate the interval endpoints,
    /// so an integrand that is unbounded but integrable at `a` or `b` is handled
    /// without special-casing — the same property `raffles` relies on for
    /// quantile-function moments.
    GaussLegendre {
        /// Nodes per subinterval.
        order: GaussOrder,
        /// Number of equal subintervals, `>= 1` (`0` is treated as `1`).
        panels: usize,
    },
}

impl QuadratureRule {
    /// Integrand evaluations this rule performs per lane, dimensionless.
    ///
    /// Fixed and data-independent — that is the defining property of this enum.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::ode::parallel::{GaussOrder, QuadratureRule};
    ///
    /// assert_eq!(QuadratureRule::Trapezoid { panels: 4 }.evaluations(), 5);
    /// assert_eq!(QuadratureRule::Simpson { panels: 4 }.evaluations(), 9);
    /// assert_eq!(
    ///     QuadratureRule::GaussLegendre { order: GaussOrder::G5, panels: 4 }.evaluations(),
    ///     20
    /// );
    /// ```
    #[must_use]
    pub const fn evaluations(self) -> usize {
        match self {
            Self::Trapezoid { panels } => panels_of(panels) + 1,
            Self::Simpson { panels } => 2 * panels_of(panels) + 1,
            Self::GaussLegendre { order, panels } => panels_of(panels) * order.points(),
        }
    }

    /// A short human-readable label, for benchmark tables and log lines.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Trapezoid { .. } => "trapezoid",
            Self::Simpson { .. } => "simpson",
            Self::GaussLegendre { .. } => "gauss-legendre",
        }
    }
}

/// `panels`, floored at 1 — the one place the "`0` means `1`" convention lives.
const fn panels_of(panels: usize) -> usize {
    if panels == 0 {
        1
    } else {
        panels
    }
}

/// Tolerances and work limit for [`adaptive_quadrature_batch`].
///
/// # Units
///
/// `abs_tol` is in the units of the *integral* (integrand times abscissa);
/// `rel_tol` is a dimensionless ratio; `max_subdivisions` is a count.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::ode::parallel::AdaptiveSettings;
///
/// // Struct-update syntax keeps the defaults you did not mean to change.
/// let s = AdaptiveSettings { abs_tol: 1e-12, ..AdaptiveSettings::default() };
/// assert_eq!(s.rel_tol, 1e-8);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveSettings {
    /// Absolute tolerance on the whole integral, in the integral's units.
    pub abs_tol: f64,
    /// Relative tolerance, dimensionless, applied against the running estimate
    /// of the integral's magnitude.
    pub rel_tol: f64,
    /// Maximum bisections per lane before reporting
    /// [`QuadratureStatus::ToleranceNotMet`]. Also bounded by
    /// [`MAX_ADAPTIVE_DEPTH`] on depth.
    pub max_subdivisions: u32,
}

impl Default for AdaptiveSettings {
    fn default() -> Self {
        Self {
            abs_tol: 1e-10,
            rel_tol: 1e-8,
            max_subdivisions: 1_000,
        }
    }
}

/// How one lane of a quadrature batch ended.
///
/// | Variant | `last_value` | Meaning |
/// |---|---|---|
/// | [`Evaluated`](Self::Evaluated) | the integral | the rule ran to completion |
/// | [`ToleranceNotMet`](Self::ToleranceNotMet) | best estimate — **not** to tolerance | adaptive lane ran out of subdivisions |
/// | [`NotFinite`](Self::NotFinite) | `NaN` | the accumulated value was `NaN` or infinite |
/// | [`InvalidInterval`](Self::InvalidInterval) | `NaN` | a limit is not finite |
///
/// # What `Evaluated` does and does not claim
///
/// For [`adaptive_quadrature_batch`] it means the requested tolerance was met.
/// For a fixed [`QuadratureRule`] it means only that every node evaluated to a
/// finite number and the sum is finite — **it makes no accuracy claim at all**,
/// because a fixed rule has no way to know. Choosing enough panels for the
/// integrand is the caller's responsibility, and
/// [`QuadratureSolution::error_estimate`] is `NaN` on that path for exactly this
/// reason.
///
/// # Units
///
/// Dimensionless — a status tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuadratureStatus {
    /// The rule ran to completion with a finite result.
    Evaluated,
    /// Adaptive only: the subdivision budget or [`MAX_ADAPTIVE_DEPTH`] was
    /// reached before the tolerance. The reported value is the best estimate
    /// available and is **not** claimed to meet the tolerance.
    ToleranceNotMet,
    /// The accumulated value is `NaN` or infinite — either the integrand
    /// returned a non-finite sample, or the sum overflowed.
    NotFinite,
    /// A limit of the interval was infinite or `NaN`, so there is nothing to
    /// integrate over.
    InvalidInterval,
}

impl QuadratureStatus {
    /// Whether this status means the lane produced a usable value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::ode::parallel::QuadratureStatus;
    ///
    /// assert!(QuadratureStatus::Evaluated.is_evaluated());
    /// assert!(!QuadratureStatus::ToleranceNotMet.is_evaluated());
    /// ```
    #[must_use]
    pub fn is_evaluated(self) -> bool {
        matches!(self, Self::Evaluated)
    }

    /// A short human-readable label, for log lines and failure reports.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Evaluated => "evaluated",
            Self::ToleranceNotMet => "tolerance-not-met",
            Self::NotFinite => "not-finite",
            Self::InvalidInterval => "invalid-interval",
        }
    }
}

/// The outcome of a single quadrature lane.
///
/// The fields are private on purpose, on the same reasoning as
/// [`OdeLaneSolution`]: [`Self::value`] returns `Option<f64>` and hands back
/// `Some` only for a lane that ran to completion, while the raw number is behind
/// the deliberately-named [`Self::last_value`].
///
/// `Copy`, so it can be read out of a [`QuadratureBatch`] without cloning.
///
/// # Units
///
/// [`Self::value`], [`Self::last_value`] and [`Self::error_estimate`] are in the
/// integral's units (integrand times abscissa); [`Self::evaluations`] is a
/// dimensionless count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuadratureSolution {
    value: f64,
    error_estimate: f64,
    evaluations: u32,
    status: QuadratureStatus,
}

impl QuadratureSolution {
    /// The integral, if this lane ran to completion.
    ///
    /// `None` for every failure status. This is the accessor to reach for; the
    /// `Option` is the point.
    #[must_use]
    pub fn value(&self) -> Option<f64> {
        if self.status.is_evaluated() {
            Some(self.value)
        } else {
            None
        }
    }

    /// The last value the rule accumulated, complete or not.
    ///
    /// **The integral only when [`Self::status`] is
    /// [`QuadratureStatus::Evaluated`].** For
    /// [`QuadratureStatus::ToleranceNotMet`] this is the best estimate the
    /// adaptive lane reached and is offered for diagnosis — for deciding whether
    /// the lane was close or wildly off. For the other two statuses it is `NaN`,
    /// because there is no honest number to report.
    #[must_use]
    pub fn last_value(&self) -> f64 {
        self.value
    }

    /// Estimated absolute error, in the integral's units.
    ///
    /// Meaningful **only** for [`adaptive_quadrature_batch`], where it is the
    /// summed Richardson estimate `|S_2 - S_1| / 15` over the accepted
    /// sub-intervals. `NaN` for every fixed [`QuadratureRule`], because a fixed
    /// rule has no error estimate to give and returning `0.0` would be a
    /// falsehood.
    #[must_use]
    pub fn error_estimate(&self) -> f64 {
        self.error_estimate
    }

    /// Integrand evaluations this lane performed, dimensionless.
    ///
    /// Fixed and predictable for a [`QuadratureRule`]; data-dependent for the
    /// adaptive path, where it is the honest measure of how hard the integrand
    /// was and the source of the load imbalance work-stealing exists to absorb.
    #[must_use]
    pub fn evaluations(&self) -> u32 {
        self.evaluations
    }

    /// How this lane ended.
    #[must_use]
    pub fn status(&self) -> QuadratureStatus {
        self.status
    }

    /// Whether this lane produced a usable value.
    #[must_use]
    pub fn evaluated(&self) -> bool {
        self.status.is_evaluated()
    }

    /// A failed lane with no honest number to report.
    #[inline]
    fn failed(status: QuadratureStatus, evaluations: u32) -> Self {
        Self {
            value: f64::NAN,
            error_estimate: f64::NAN,
            evaluations,
            status,
        }
    }
}

/// A batch of `N` quadrature outcomes, in the same order as the intervals handed
/// in.
///
/// Lane `i` of the result corresponds to `intervals[i]`, always — the parallel
/// path preserves order.
///
/// # Getting values out
///
/// - [`Self::values`] — all-or-nothing. `Ok(Vec<f64>)` only when every lane ran
///   to completion; otherwise `Err(`[`QuadratureBatchFailure`]`)`.
/// - [`Self::solutions`] — per-lane, when the caller wants to handle failures
///   individually.
///
/// # Units
///
/// See [`QuadratureSolution`].
#[derive(Debug, Clone, PartialEq)]
pub struct QuadratureBatch {
    solutions: Vec<QuadratureSolution>,
}

impl QuadratureBatch {
    /// Every lane's outcome, in input order.
    #[must_use]
    pub fn solutions(&self) -> &[QuadratureSolution] {
        &self.solutions
    }

    /// Consume the batch and take the outcomes.
    #[must_use]
    pub fn into_solutions(self) -> Vec<QuadratureSolution> {
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
    pub fn get(&self, i: usize) -> Option<QuadratureSolution> {
        self.solutions.get(i).copied()
    }

    /// Whether every lane ran to completion. Vacuously `true` when empty.
    #[must_use]
    pub fn all_evaluated(&self) -> bool {
        self.solutions.iter().all(QuadratureSolution::evaluated)
    }

    /// How many lanes failed, dimensionless.
    #[must_use]
    pub fn failure_count(&self) -> usize {
        self.solutions.iter().filter(|s| !s.evaluated()).count()
    }

    /// The first failing lane and its outcome, if any.
    #[must_use]
    pub fn first_failure(&self) -> Option<(usize, QuadratureSolution)> {
        self.solutions
            .iter()
            .enumerate()
            .find(|(_, s)| !s.evaluated())
            .map(|(i, s)| (i, *s))
    }

    /// Every failing lane, as `(index, outcome)` pairs.
    ///
    /// Allocates, so prefer [`Self::first_failure`] on a hot path.
    #[must_use]
    pub fn failures(&self) -> Vec<(usize, QuadratureSolution)> {
        self.solutions
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.evaluated())
            .map(|(i, s)| (i, *s))
            .collect()
    }

    /// Total integrand evaluations over every lane, dimensionless.
    #[must_use]
    pub fn total_evaluations(&self) -> u64 {
        self.solutions
            .iter()
            .map(|s| u64::from(s.evaluations))
            .sum()
    }

    /// All integrals, or an error naming the failures — the all-or-nothing path.
    ///
    /// # Returns
    ///
    /// `Ok(v)` with `v[i]` the integral over `intervals[i]`, in the integral's
    /// units, when every lane ran to completion. Otherwise
    /// `Err(`[`QuadratureBatchFailure`]`)`. **No `Vec` of plausible-looking
    /// numbers is ever returned for a batch that contained a failure.**
    ///
    /// An empty batch returns `Ok(vec![])`.
    ///
    /// # Errors
    ///
    /// [`QuadratureBatchFailure`] when one or more lanes failed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::compute::ComputeBackend;
    /// use outram_foam_basic_lib::ode::parallel::{
    ///     quadrature_batch, QuadratureInterval, QuadratureRule, QuadratureStatus,
    /// };
    ///
    /// // Lane 1's upper limit is not finite.
    /// let intervals = [
    ///     QuadratureInterval::new(0.0, 1.0),
    ///     QuadratureInterval::new(0.0, f64::INFINITY),
    /// ];
    /// let batch = quadrature_batch(
    ///     &intervals,
    ///     QuadratureRule::Simpson { panels: 8 },
    ///     ComputeBackend::Serial,
    ///     |_, x| x * x,
    /// );
    ///
    /// let err = batch.values().expect_err("lane 1 has no finite interval");
    /// assert_eq!(err.failure_count, 1);
    /// assert_eq!(err.first_index, 1);
    /// assert_eq!(err.first_status, QuadratureStatus::InvalidInterval);
    ///
    /// // The good lane is still individually readable, and Simpson is exact
    /// // for x^2.
    /// let v = batch.solutions()[0].value().unwrap();
    /// assert!((v - 1.0 / 3.0).abs() < 1e-15);
    /// assert!(batch.solutions()[1].value().is_none());
    /// ```
    pub fn values(&self) -> Result<Vec<f64>, QuadratureBatchFailure> {
        if let Some((i, s)) = self.first_failure() {
            return Err(QuadratureBatchFailure {
                total: self.solutions.len(),
                failure_count: self.failure_count(),
                first_index: i,
                first_status: s.status(),
                first_evaluations: s.evaluations(),
            });
        }
        Ok(self.solutions.iter().map(|s| s.value).collect())
    }
}

/// One or more lanes of a [`QuadratureBatch`] failed.
///
/// Returned by [`QuadratureBatch::values`]. As [`OdeEnsembleFailure`], it names
/// both the scale of the problem and a specific lane to look at.
///
/// # Units
///
/// All counts and indices are dimensionless.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "{failure_count} of {total} quadrature lanes failed; \
     first failure at lane {first_index} with status {first_status:?} \
     after {first_evaluations} integrand evaluations"
)]
pub struct QuadratureBatchFailure {
    /// Number of lanes in the batch.
    pub total: usize,
    /// Number of lanes that failed.
    pub failure_count: usize,
    /// Index of the first failing lane.
    pub first_index: usize,
    /// Why that lane failed.
    pub first_status: QuadratureStatus,
    /// Integrand evaluations that lane performed.
    pub first_evaluations: u32,
}

// ── Quadrature entry points ──────────────────────────────────────────────────

/// Evaluate `N` independent definite integrals with a **fixed rule**, on the
/// chosen backend.
///
/// The GPU-shaped half of this module: every lane performs exactly
/// [`QuadratureRule::evaluations`] integrand calls with no data-dependent
/// branching. Use it when the integrand is smooth and the panel count can be
/// chosen once for the whole batch — a band-averaged cross section, a
/// face-integrated flux, a cell-integrated source term.
///
/// # Arguments
///
/// - `intervals` — one [`QuadratureInterval`] per lane.
/// - `rule` — the rule applied to every lane. [`QuadratureRule::GaussLegendre`]
///   is the default worth reaching for on a smooth integrand.
/// - `backend` — requested backend. What actually runs is
///   [`quadrature_backend_for`] applied to it; a batch below
///   [`QUADRATURE_MIN_INTERVALS`] runs serially. None of the degradations
///   changes the answer.
/// - `f` — the integrand. `f(i, x)` must return lane `i`'s integrand at abscissa
///   `x`. It **must be a pure deterministic function of its arguments** — see
///   the module-level "Determinism" section. It is called from multiple threads
///   on the `CpuMulti` path, hence the `Sync` bound; the bound is present in
///   both feature builds so that enabling `parallel` never changes a public
///   signature.
///
/// # Returns
///
/// A [`QuadratureBatch`] with one [`QuadratureSolution`] per interval, in input
/// order.
///
/// # Determinism
///
/// Bit-for-bit identical across backends and thread counts. The sum within a
/// lane is sequential and is never split across threads — see the module-level
/// "Determinism" section for why that restriction is deliberate.
///
/// # Verification
///
/// *Methodology.* Three oracles, all exact rather than another implementation.
/// (1) *Polynomial exactness*: an `n`-point Gauss-Legendre rule must integrate
/// every monomial `x^d` for `d <= 2n - 1` exactly, Simpson must be exact for
/// `d <= 3` and trapezoid for `d <= 1`; checked over `[0, 1]` and `[-2, 3]`
/// against the closed form `(b^(d+1) - a^(d+1)) / (d + 1)`. (2) *Published
/// nodes*: the computed 8-point nodes and weights against the Abramowitz &
/// Stegun 25.4.30 values already carried in this workspace by
/// `crates/raffles/src/distributions.rs`. (3) *Transcendental reference*:
/// `integral of exp(-x) sin(x) from 0 to pi = (1 + exp(-pi)) / 2`. Pass
/// criteria: `< 1e-13` relative for exactness, `< 1e-15` absolute against A&S,
/// `< 1e-12` absolute for the transcendental.
///
/// *Results, measured 2026-08-13 by `gauss_legendre_is_exact_to_its_degree`,
/// `gauss_nodes_match_the_in_workspace_abramowitz_stegun_values`,
/// `simpson_and_trapezoid_are_exact_to_their_degree` and
/// `fixed_rules_match_a_transcendental_reference` in `parallel/tests.rs`,
/// release build:*
///
/// - Exactness sweep (`G2`..`G8`, degrees 0 to 15, both intervals): worst
///   relative error **5.769990e-16**. Simpson over degrees 0 to 3:
///   **3.552714e-16**. Trapezoid over degrees 0 to 1: **0.000000e0**.
/// - Computed `G8` against A&S 25.4.30: worst node difference
///   **1.110223e-16**, worst weight difference **1.249001e-16**, weights
///   summing to **2.00000000000000000**.
/// - Transcendental reference, closed form **0.52160695913188615**: `G8` over 8
///   panels (64 evaluations) gave **0.52160695913188615**, error
///   **0.000000e0**; Simpson over 64 panels (129 evaluations) error
///   **4.206809e-9**; trapezoid over 128 panels (129 evaluations) error
///   **5.236767e-5**.
///
/// *Interpretation.* The Gauss nodes and weights are correct to the last bit
/// `f64` holds — agreeing with a published table to within one unit in the last
/// place while being computed independently of it — the composite mapping onto
/// arbitrary intervals is correct, and the order hierarchy behaves as theory
/// requires: `G8` reaches the rounding floor on a smooth transcendental at half
/// the evaluations where a 64-panel Simpson is still nine orders away and a
/// trapezoid four orders beyond that.
///
/// # Example — the `uom` boundary
///
/// The batch is dimensionless, and the caller converts at its edge. These lanes
/// integrate a linearly-ramping electrical power over time to recover an
/// `Energy`:
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::ode::parallel::{
///     quadrature_batch, GaussOrder, QuadratureInterval, QuadratureRule,
/// };
/// use uom::si::f64::{Energy, Power, Time};
/// use uom::si::energy::joule;
/// use uom::si::power::watt;
/// use uom::si::time::second;
///
/// // P(t) = P0 (1 - t / T), shutting down linearly over the window.
/// let p0 = Power::new::<watt>(2.0e6);
/// let window = Time::new::<second>(30.0);
///
/// // Convert in: limits in seconds, as plain f64.
/// let intervals = [
///     QuadratureInterval::new(0.0, window.get::<second>()),
///     QuadratureInterval::new(0.0, 0.5 * window.get::<second>()),
/// ];
///
/// let batch = quadrature_batch(
///     &intervals,
///     QuadratureRule::GaussLegendre { order: GaussOrder::G4, panels: 4 },
///     ComputeBackend::CpuMulti,
///     |_, t| p0.get::<watt>() * (1.0 - t / window.get::<second>()),
/// );
///
/// // Convert out: back to typed energies.
/// let energies: Vec<Energy> = batch
///     .values()
///     .expect("both lanes evaluate")
///     .into_iter()
///     .map(Energy::new::<joule>)
///     .collect();
///
/// // Closed form over [0, T] is P0 T / 2; over [0, T/2] it is 3 P0 T / 8.
/// assert!((energies[0].get::<joule>() - 0.5 * 2.0e6 * 30.0).abs() < 1e-6);
/// assert!((energies[1].get::<joule>() - 0.375 * 2.0e6 * 30.0).abs() < 1e-6);
/// ```
#[must_use]
pub fn quadrature_batch<F>(
    intervals: &[QuadratureInterval],
    rule: QuadratureRule,
    backend: ComputeBackend,
    f: F,
) -> QuadratureBatch
where
    F: Fn(usize, f64) -> f64 + Sync,
{
    quadrature_batch_min(intervals, rule, backend, QUADRATURE_MIN_INTERVALS, f)
}

/// [`quadrature_batch`] with the size floor supplied by the caller; see
/// [`integrate_ensemble_min`] for why the `_min` variants exist.
pub(crate) fn quadrature_batch_min<F>(
    intervals: &[QuadratureInterval],
    rule: QuadratureRule,
    backend: ComputeBackend,
    min_intervals: usize,
    f: F,
) -> QuadratureBatch
where
    F: Fn(usize, f64) -> f64 + Sync,
{
    // Gauss nodes are computed once per batch, not once per lane: the table is
    // a pure function of the order, so sharing it cannot change any lane's
    // value and it keeps the per-lane kernel free of setup cost.
    let nodes = match rule {
        QuadratureRule::GaussLegendre { order, .. } => gauss_legendre_nodes(order.points()),
        _ => Vec::new(),
    };
    let n = intervals.len();
    let solutions: Vec<QuadratureSolution> = match effective_backend(backend, n, min_intervals) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => intervals
            .par_iter()
            .enumerate()
            .map(|(i, iv)| quadrature_one(i, *iv, rule, &nodes, &f))
            .collect(),
        _ => intervals
            .iter()
            .enumerate()
            .map(|(i, iv)| quadrature_one(i, *iv, rule, &nodes, &f))
            .collect(),
    };
    QuadratureBatch { solutions }
}

/// Evaluate `N` independent definite integrals **adaptively**, on the chosen
/// backend.
///
/// Adaptive Simpson with local error control: each lane bisects wherever the
/// integrand resists a Simpson panel, and stops where it does not. Use it when
/// the integrand's difficulty is not known in advance, or differs between lanes,
/// or is concentrated in a small part of the interval — a peak in a resonance
/// integral, a boundary layer, a kink at a phase boundary.
///
/// # Why this path is CPU-only, and stays so
///
/// The subdivision pattern is decided by the integrand at run time, so
/// neighbouring lanes take different branches and perform different numbers of
/// evaluations. That is exactly the control-flow divergence SIMT hardware
/// handles worst — a GPU implementation would serialise the divergent branches
/// and need a per-lane work stack in device memory. Batching it across CPU cores
/// costs nothing and works; putting it on a GPU would be a large amount of code
/// for an unclear win. **It is deliberately CPU-only**, and this is the reason.
///
/// The CPU path is still fully hybrid: it takes a [`ComputeBackend`] and threads
/// across lanes exactly like every other kernel here.
///
/// # Arguments
///
/// - `intervals` — one [`QuadratureInterval`] per lane.
/// - `settings` — tolerances and the subdivision budget; see
///   [`AdaptiveSettings`].
/// - `backend` — requested backend; see [`quadrature_backend_for`]. The
///   [`QUADRATURE_MIN_INTERVALS`] floor is shared with [`quadrature_batch`] and
///   was measured on that path, so it is a conservative assumption here rather
///   than a measurement.
/// - `f` — the integrand, `f(i, x)`. Must be pure and deterministic; called from
///   multiple threads on the `CpuMulti` path.
///
/// # Returns
///
/// A [`QuadratureBatch`]. A lane that met its tolerance reports
/// [`QuadratureStatus::Evaluated`] and a meaningful
/// [`QuadratureSolution::error_estimate`]; a lane that exhausted its budget
/// reports [`QuadratureStatus::ToleranceNotMet`] and its best estimate, and is
/// excluded from [`QuadratureBatch::values`].
///
/// # Determinism
///
/// Bit-for-bit identical across backends and thread counts. The subdivision
/// order within a lane is a deterministic function of the integrand, and no
/// lane's arithmetic is split across threads.
///
/// # Verification
///
/// *Methodology.* Run against three integrands with closed forms, two of them
/// deliberately awkward for a uniform panel layout:
/// `integral of exp(-x) sin(x) from 0 to pi = (1 + exp(-pi)) / 2`;
/// `integral of sqrt(x) from 0 to 1 = 2/3`, which is bounded but has an
/// infinite derivative at the lower limit; and
/// `integral of 1 / (1 + 400 (x - 1/2)^2) from 0 to 1 = atan(10) / 10`, a peak
/// occupying about a twentieth of the interval. Tolerances `abs_tol = 1e-11`,
/// `rel_tol = 1e-10`, `max_subdivisions = 100 000`. Pass criteria: absolute
/// error below `1e-9`; every lane [`QuadratureStatus::Evaluated`]; and the
/// reported [`QuadratureSolution::error_estimate`] no more than 100x smaller
/// than the true error, since an estimate that badly understates the error
/// would be worse than none.
///
/// *Results, measured 2026-08-13 by `adaptive_matches_closed_forms` in
/// `parallel/tests.rs`, release build:*
///
/// | Integrand | Value | Error | Reported estimate | Evaluations |
/// |---|---|---|---|---|
/// | `exp(-x) sin(x)` on `[0, pi]` | 0.52160695913188759 | 1.443290e-15 | 2.034556e-11 | 469 |
/// | `sqrt(x)` on `[0, 1]` | 0.66666666666664931 | 1.731948e-14 | 2.619949e-11 | 1057 |
/// | narrow peak on `[0, 1]` | 0.14711276743037432 | 8.604228e-16 | 2.682116e-11 | 1225 |
///
/// *Interpretation.* All three land at or near the rounding floor, four to five
/// orders better than the requested `1e-11`, and the reported estimate is
/// conservative in every case — it overstates the true error by three to four
/// orders rather than understating it, which is the safe direction for an error
/// bound. The evaluation counts are the point of the adaptive path and the
/// source of the load imbalance work-stealing exists to absorb: 469 against
/// 1 225 for problems posed identically, a 2.6x spread decided entirely by the
/// integrand. `sqrt(x)` is the informative case — its infinite endpoint
/// derivative makes a uniform panel layout converge slowly, and the adaptive
/// path instead concentrates its subdivisions near `x = 0`.
///
/// # Limitations
///
/// **An integrand that is unbounded at an interval endpoint is not supported
/// here.** Adaptive Simpson evaluates `f(a)` and `f(b)` on its very first step,
/// so an integrable singularity sitting exactly on a limit — `ln(x)` or
/// `1/sqrt(x)` at `x = 0` — produces a non-finite first estimate and the lane
/// reports [`QuadratureStatus::NotFinite`]. This inverts the naive expectation
/// that the adaptive path is the more capable one: for an endpoint singularity
/// reach for [`quadrature_batch`] with
/// [`QuadratureRule::GaussLegendre`] instead, whose nodes are strictly interior
/// and never touch the limits. That is exactly the property `raffles` relies on
/// for its quantile-function moments. A singularity in the *interior* of the
/// interval is also not handled: it will exhaust the subdivision budget and
/// report [`QuadratureStatus::ToleranceNotMet`], which is at least honest.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::ode::parallel::{
///     adaptive_quadrature_batch, AdaptiveSettings, QuadratureInterval,
/// };
///
/// // Three lanes of `integral of exp(-a x) from 0 to 1` = (1 - exp(-a)) / a.
/// let a = [1.0_f64, 5.0, 20.0];
/// let intervals: Vec<QuadratureInterval> =
///     (0..3).map(|_| QuadratureInterval::new(0.0, 1.0)).collect();
///
/// let batch = adaptive_quadrature_batch(
///     &intervals,
///     AdaptiveSettings::default(),
///     ComputeBackend::CpuMulti,
///     |i, x| (-a[i] * x).exp(),
/// );
///
/// for (i, v) in batch.values().expect("all lanes meet tolerance").iter().enumerate() {
///     let exact = (1.0 - (-a[i]).exp()) / a[i];
///     assert!((v - exact).abs() < 1e-10, "lane {i}: {v} vs {exact}");
/// }
/// ```
#[must_use]
pub fn adaptive_quadrature_batch<F>(
    intervals: &[QuadratureInterval],
    settings: AdaptiveSettings,
    backend: ComputeBackend,
    f: F,
) -> QuadratureBatch
where
    F: Fn(usize, f64) -> f64 + Sync,
{
    adaptive_quadrature_batch_min(intervals, settings, backend, QUADRATURE_MIN_INTERVALS, f)
}

/// [`adaptive_quadrature_batch`] with the size floor supplied by the caller; see
/// [`integrate_ensemble_min`] for why the `_min` variants exist.
pub(crate) fn adaptive_quadrature_batch_min<F>(
    intervals: &[QuadratureInterval],
    settings: AdaptiveSettings,
    backend: ComputeBackend,
    min_intervals: usize,
    f: F,
) -> QuadratureBatch
where
    F: Fn(usize, f64) -> f64 + Sync,
{
    let n = intervals.len();
    let solutions: Vec<QuadratureSolution> = match effective_backend(backend, n, min_intervals) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => intervals
            .par_iter()
            .enumerate()
            // No `min_len` floor: adaptive evaluation counts vary by orders of
            // magnitude between lanes.
            .map(|(i, iv)| adaptive_one(i, *iv, settings, &f))
            .collect(),
        _ => intervals
            .iter()
            .enumerate()
            .map(|(i, iv)| adaptive_one(i, *iv, settings, &f))
            .collect(),
    };
    QuadratureBatch { solutions }
}

// ── Per-lane quadrature kernels — one implementation, both backends ──────────

/// Sort a lane's limits and record the orientation sign.
///
/// Returns `Err(status)` when there is nothing to integrate: a non-finite limit,
/// or a zero-length interval (which is a *success* with value zero, handled by
/// the caller).
#[inline]
fn orient(iv: QuadratureInterval) -> Result<(f64, f64, f64), QuadratureStatus> {
    if !iv.a.is_finite() || !iv.b.is_finite() {
        return Err(QuadratureStatus::InvalidInterval);
    }
    if iv.a == iv.b {
        return Err(QuadratureStatus::Evaluated);
    }
    if iv.b > iv.a {
        Ok((iv.a, iv.b, 1.0))
    } else {
        Ok((iv.b, iv.a, -1.0))
    }
}

/// Evaluate one lane with a fixed rule — the per-lane kernel both backends call.
#[inline]
fn quadrature_one<F>(
    i: usize,
    iv: QuadratureInterval,
    rule: QuadratureRule,
    nodes: &[(f64, f64)],
    f: &F,
) -> QuadratureSolution
where
    F: Fn(usize, f64) -> f64,
{
    let (lo, hi, sign) = match orient(iv) {
        Ok(t) => t,
        Err(QuadratureStatus::Evaluated) => {
            return QuadratureSolution {
                value: 0.0,
                error_estimate: f64::NAN,
                evaluations: 0,
                status: QuadratureStatus::Evaluated,
            }
        }
        Err(status) => return QuadratureSolution::failed(status, 0),
    };

    let value = match rule {
        QuadratureRule::Trapezoid { panels } => {
            let p = panels_of(panels);
            let h = (hi - lo) / p as f64;
            let mut sum = 0.5 * (f(i, lo) + f(i, hi));
            for k in 1..p {
                sum += f(i, lo + k as f64 * h);
            }
            h * sum
        }
        QuadratureRule::Simpson { panels } => {
            let p = panels_of(panels);
            let sub = 2 * p;
            let h = (hi - lo) / sub as f64;
            let mut sum = f(i, lo) + f(i, hi);
            for k in 1..sub {
                let w = if k % 2 == 1 { 4.0 } else { 2.0 };
                sum += w * f(i, lo + k as f64 * h);
            }
            h / 3.0 * sum
        }
        QuadratureRule::GaussLegendre { panels, .. } => {
            let p = panels_of(panels);
            let width = (hi - lo) / p as f64;
            let half = 0.5 * width;
            let mut total = 0.0;
            for k in 0..p {
                let mid = lo + (k as f64 + 0.5) * width;
                let mut panel = 0.0;
                for &(node, weight) in nodes {
                    panel += weight * f(i, mid + half * node);
                }
                total += half * panel;
            }
            total
        }
    };

    let evaluations = rule.evaluations().min(u32::MAX as usize) as u32;
    if value.is_finite() {
        QuadratureSolution {
            value: sign * value,
            // A fixed rule has no error estimate to offer; `0.0` would be a lie.
            error_estimate: f64::NAN,
            evaluations,
            status: QuadratureStatus::Evaluated,
        }
    } else {
        QuadratureSolution::failed(QuadratureStatus::NotFinite, evaluations)
    }
}

/// One pending sub-interval of the adaptive lane, held on an explicit stack.
///
/// An explicit stack rather than recursion so the depth is bounded by data the
/// kernel owns rather than by the thread's stack size — a `rayon` worker's stack
/// is not the caller's, and a pathological integrand must not be able to
/// overflow it.
#[derive(Debug, Clone, Copy)]
struct AdaptiveSegment {
    a: f64,
    b: f64,
    fa: f64,
    fm: f64,
    fb: f64,
    /// Simpson estimate over `[a, b]` using `fa`, `fm`, `fb`.
    whole: f64,
    /// Local tolerance allotted to this sub-interval.
    tol: f64,
    depth: u32,
}

/// Evaluate one lane adaptively — the per-lane kernel both backends call.
///
/// Adaptive Simpson: compare the Simpson estimate over `[a, b]` with the sum of
/// the estimates over its two halves; accept when they agree to the local
/// tolerance, with the Richardson correction `(S_2 - S_1) / 15` applied, and
/// bisect otherwise with the tolerance split between the halves.
#[inline]
fn adaptive_one<F>(
    i: usize,
    iv: QuadratureInterval,
    settings: AdaptiveSettings,
    f: &F,
) -> QuadratureSolution
where
    F: Fn(usize, f64) -> f64,
{
    let (lo, hi, sign) = match orient(iv) {
        Ok(t) => t,
        Err(QuadratureStatus::Evaluated) => {
            return QuadratureSolution {
                value: 0.0,
                error_estimate: 0.0,
                evaluations: 0,
                status: QuadratureStatus::Evaluated,
            }
        }
        Err(status) => return QuadratureSolution::failed(status, 0),
    };

    let mut evaluations: u32 = 0;
    let eval = |x: f64, evaluations: &mut u32| {
        *evaluations = evaluations.saturating_add(1);
        f(i, x)
    };

    let fa = eval(lo, &mut evaluations);
    let fb = eval(hi, &mut evaluations);
    let m0 = 0.5 * (lo + hi);
    let fm = eval(m0, &mut evaluations);
    let whole = (hi - lo) / 6.0 * (fa + 4.0 * fm + fb);

    // The tolerance is absolute plus relative-to-the-first-estimate. Using the
    // coarse whole-interval estimate as the scale is the standard choice: it is
    // available before any subdivision and cannot itself depend on the
    // subdivision path, which is what keeps the lane deterministic.
    let scale_tol = settings.abs_tol + settings.rel_tol * whole.abs();

    let mut stack: Vec<AdaptiveSegment> = Vec::with_capacity(32);
    stack.push(AdaptiveSegment {
        a: lo,
        b: hi,
        fa,
        fm,
        fb,
        whole,
        tol: scale_tol,
        depth: 0,
    });

    let mut total = 0.0_f64;
    let mut error = 0.0_f64;
    let mut splits: u32 = 0;
    let mut budget_exhausted = false;

    while let Some(seg) = stack.pop() {
        let lm = 0.5 * (seg.a + seg.b);
        let ml = 0.5 * (seg.a + lm);
        let mr = 0.5 * (lm + seg.b);
        let fml = eval(ml, &mut evaluations);
        let fmr = eval(mr, &mut evaluations);

        let h = seg.b - seg.a;
        let left = h / 12.0 * (seg.fa + 4.0 * fml + seg.fm);
        let right = h / 12.0 * (seg.fm + 4.0 * fmr + seg.fb);
        let refined = left + right;
        let delta = refined - seg.whole;

        let out_of_budget = splits >= settings.max_subdivisions || seg.depth >= MAX_ADAPTIVE_DEPTH;
        if delta.abs() <= 15.0 * seg.tol || out_of_budget {
            if out_of_budget && delta.abs() > 15.0 * seg.tol {
                budget_exhausted = true;
            }
            total += refined + delta / 15.0;
            error += (delta / 15.0).abs();
            continue;
        }

        splits = splits.saturating_add(1);
        let half_tol = 0.5 * seg.tol;
        stack.push(AdaptiveSegment {
            a: seg.a,
            b: lm,
            fa: seg.fa,
            fm: fml,
            fb: seg.fm,
            whole: left,
            tol: half_tol,
            depth: seg.depth + 1,
        });
        stack.push(AdaptiveSegment {
            a: lm,
            b: seg.b,
            fa: seg.fm,
            fm: fmr,
            fb: seg.fb,
            whole: right,
            tol: half_tol,
            depth: seg.depth + 1,
        });
    }

    if !total.is_finite() {
        return QuadratureSolution::failed(QuadratureStatus::NotFinite, evaluations);
    }

    QuadratureSolution {
        value: sign * total,
        error_estimate: error,
        evaluations,
        status: if budget_exhausted {
            QuadratureStatus::ToleranceNotMet
        } else {
            QuadratureStatus::Evaluated
        },
    }
}

// ── Gauss-Legendre nodes ─────────────────────────────────────────────────────

/// Nodes and weights of the `n`-point Gauss-Legendre rule on `[-1, 1]`, in
/// ascending node order.
///
/// **Computed, not transcribed.** The nodes are the roots of the Legendre
/// polynomial `P_n`, found by Newton iteration from the standard Chebyshev-like
/// initial guess `cos(pi (k - 1/4) / (n + 1/2))`, and the weights follow from
/// `w_k = 2 / ((1 - x_k^2) P_n'(x_k)^2)`. `P_n` and `P_n'` come from Bonnet's
/// recurrence. Computing them rather than copying a printed table means the
/// values are correct to the last bit `f64` can hold at any order, with no
/// transcription to get wrong and no literature dependency to record; the
/// exactness property of the rule (degree `2n - 1`) is then a complete
/// self-check, and is what `gauss_legendre_is_exact_to_its_degree` asserts.
///
/// The result is deterministic — the same nodes on every call and every thread —
/// so sharing one table across a batch's lanes cannot change any lane's value.
///
/// # Units
///
/// Dimensionless: nodes on `[-1, 1]`, weights summing to 2.
fn gauss_legendre_nodes(n: usize) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(n);
    let nf = n as f64;
    for k in 1..=n {
        let mut x = (std::f64::consts::PI * (k as f64 - 0.25) / (nf + 0.5)).cos();
        // Newton on P_n. Converges quadratically from this guess; the cap is a
        // safety net, not the termination condition.
        for _ in 0..64 {
            let (p, dp) = legendre_p_and_dp(n, x);
            if dp == 0.0 {
                break;
            }
            let step = p / dp;
            x -= step;
            if step.abs() <= f64::EPSILON * x.abs().max(1.0) {
                break;
            }
        }
        let (_, dp) = legendre_p_and_dp(n, x);
        let w = 2.0 / ((1.0 - x * x) * dp * dp);
        out.push((x, w));
    }
    // The guess enumerates nodes from +1 downwards; ascending order is friendlier
    // to read and fixes the summation order in the kernel.
    out.reverse();
    out
}

/// `P_n(x)` and `P_n'(x)` by Bonnet's recurrence.
///
/// `P_0 = 1`, `P_1 = x`, `j P_j = (2j - 1) x P_(j-1) - (j - 1) P_(j-2)`, and
/// `P_n'(x) = n (x P_n - P_(n-1)) / (x^2 - 1)`. Valid for `|x| < 1`, which is
/// where every Gauss node lies.
fn legendre_p_and_dp(n: usize, x: f64) -> (f64, f64) {
    if n == 0 {
        return (1.0, 0.0);
    }
    if n == 1 {
        return (x, 1.0);
    }
    let mut p_prev = 1.0_f64;
    let mut p = x;
    for j in 2..=n {
        let jf = j as f64;
        let p_next = ((2.0 * jf - 1.0) * x * p - (jf - 1.0) * p_prev) / jf;
        p_prev = p;
        p = p_next;
    }
    let dp = n as f64 * (x * p - p_prev) / (x * x - 1.0);
    (p, dp)
}
