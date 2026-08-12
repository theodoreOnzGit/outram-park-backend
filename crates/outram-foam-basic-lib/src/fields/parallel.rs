// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
//   src/OpenFOAM/fields/Fields/Field/Field.C
//   src/OpenFOAM/fields/GeometricFields/GeometricField/GeometricFieldFunctions.C
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! Backend-dispatched kernels for the element-wise field algebra.
//!
//! # What this module is for
//!
//! A finite-volume solver spends most of a timestep doing *element-wise*
//! arithmetic on fields that carry one value per mesh cell or per mesh face:
//! adding an explicit source to a residual, scaling by a relaxation factor,
//! forming `rho*U`, advancing `phi += dt*ddt(phi)`, and taking a norm to decide
//! whether the outer loop has converged. Each of those touches every cell,
//! several times per timestep, so on a large mesh they are the operations worth
//! spreading across CPU cores.
//!
//! **The arithmetic is identical to the serial operators** in
//! [`crate::fields::field`], [`crate::fields::vol_field`] and
//! [`crate::fields::surface_field`]. Only the execution strategy differs — and,
//! for the *reductions* only, the order in which floating-point values are
//! summed (see "Reduction determinism" below).
//!
//! # One entry point per operation
//!
//! Hybrid execution here means **dispatch, not two APIs**. There is exactly one
//! public function per operation and the backend is a parameter:
//!
//! ```rust
//! use outram_foam_basic_lib::compute::ComputeBackend;
//! use outram_foam_basic_lib::fields::field::Field;
//! use outram_foam_basic_lib::fields::parallel;
//!
//! let a = Field::uniform(1_000, 2.0_f64);
//! let b = Field::uniform(1_000, 3.0_f64);
//!
//! // Same function, different backend — no `add_parallel` twin exists.
//! let c_ref = parallel::add(ComputeBackend::Serial, &a, &b);
//! let c_mt = parallel::add(ComputeBackend::CpuMulti, &a, &b);
//! assert_eq!(c_ref.as_slice(), c_mt.as_slice());
//! ```
//!
//! The size threshold that decides whether [`ComputeBackend::CpuMulti`] actually
//! spreads the work is **one named, documented function** —
//! [`should_parallelise`], reading [`field_parallel_crossover`] — never an
//! `if n > …` scattered through the operators. Retuning the policy is a
//! one-place edit.
//!
//! # Cargo features
//!
//! Multi-threading lives behind the crate's **`parallel`** feature (rayon),
//! which is **off by default** so the default build stays dependency-light and
//! Android-clean. The public API in this module is the *same either way*: with
//! the feature off, [`ComputeBackend::CpuMulti`] and [`ComputeBackend::Gpu`]
//! transparently run the serial path and every result is unchanged. Nothing
//! here is target-gated — rayon is pure Rust, so with `--features parallel` this
//! module builds and runs on Android/Termux (`target_os = "android"`) too;
//! [`std::thread::available_parallelism`] works there and a phone simply
//! reports fewer cores.
//!
//! # The `Gpu` backend
//!
//! There is **no GPU kernel for field algebra yet**. [`ComputeBackend::Gpu`] is
//! accepted and routed to the best available CPU path (multi-threaded when the
//! `parallel` feature is on, serial otherwise). This is stated rather than
//! silently pretended; when a GPU field kernel exists it is wired in here and
//! nothing about the call sites changes.
//!
//! # Which thread pool
//!
//! The kernels use rayon's **global** pool. A caller that wants a dedicated,
//! explicitly sized pool does not need a second API — bind one with
//! [`rayon::ThreadPool::install`] and every call inside routes to it:
//!
//! ```text
//! // Sketch — requires the `parallel` feature, so it is shown rather than run
//! // as a doctest (rayon is not a dependency of the default build at all).
//! let pool = rayon::ThreadPoolBuilder::new().num_threads(8).build().unwrap();
//! pool.install(|| parallel::axpy_assign(ComputeBackend::CpuMulti, &mut y, dt, &ddt));
//! ```
//!
//! The `vv_reduction_is_independent_of_thread_count` test in this module does
//! exactly this and asserts the answer is unchanged.
//!
//! The reductions are written so that the answer does **not** depend on which
//! pool or how many threads it has — see below.
//!
//! # Units
//!
//! This layer is deliberately **unit-agnostic**, exactly like the serial field
//! layer it mirrors (see the [`crate::fields`] module docs): a [`Field`] carries
//! bare `f64` / [`Vector3`] / [`Tensor`](crate::primitives::Tensor) /
//! [`SymmTensor`](crate::primitives::SymmTensor) values in whatever SI units the
//! caller assigned them. No `uom` quantity is stripped here because none is
//! present at this layer — this crate's `uom` discipline lives in the
//! thermophysics layer and nothing in this module weakens it. Each function's
//! doc states the units it implies. The one place a physical unit is
//! unavoidable is [`vol_integral`], which multiplies by the mesh's cell volumes
//! in `m^3` and therefore returns `[phi]*m^3`.
//!
//! # Field `name` strings never grow here
//!
//! Every operation that returns a [`VolField`] or [`SurfaceField`] copies the
//! **left operand's** `name` verbatim and never composes a new one. This is not
//! a style preference. A solver that reassigns a persistent field from an
//! expression containing itself (`rho = rho + div(phi)`) would double the `name`
//! string every timestep under compositional naming — `2^step` growth, invisible
//! in the field data. That exact bug once drove this crate's
//! `compressible_lid_cavity` test to 24 GB and a SIGTERM. See the crate
//! `CLAUDE.md` "Critical translation gotcha", the matching notes in
//! [`crate::fields::vol_field`], and the `name_does_not_grow_*` regression tests
//! in this module, which reassign a field from an expression containing itself
//! 64 times and assert the name length is unchanged.
//!
//! # Reduction determinism
//!
//! The reductions ([`sum`], [`l2_norm`], [`dot`], [`vol_integral`], …) use a
//! **fixed-chunk tree reduction**: the slice is cut into consecutive chunks of
//! exactly [`REDUCTION_CHUNK`] elements, each chunk is summed sequentially in
//! index order, and the per-chunk partial sums are then combined sequentially in
//! index order on the calling thread.
//!
//! That buys a strong and deliberate guarantee:
//!
//! - the parallel reduction depends **only** on the data and on the
//!   compile-time constant [`REDUCTION_CHUNK`];
//! - it is therefore **bit-reproducible run to run** and **identical for any
//!   thread count** — unlike a work-stealing `par_iter().sum()`, whose
//!   accumulation tree depends on how rayon happened to split the work that
//!   run;
//! - it is **not** bit-identical to the serial left-to-right sum, because
//!   floating-point addition is not associative. The two differ in the last
//!   bits; the measured worst-case deviation is recorded in the V&V test
//!   `vv_parallel_sum_matches_serial_within_tolerance`.
//!
//! [`min`] and [`max`] *are* bit-identical to the serial fold, because `min` and
//! `max` are associative.
//!
//! [`ComputeBackend::Serial`] is the **deterministic trusted reference** — the
//! oracle every parallel result in this module is checked against, matching the
//! convention in `outram-mc-libs` (`src/physics/compute.rs`) and `boon-lay`
//! (`src/compute.rs`).
//!
//! # Crossover: parallel is *slower* on small fields
//!
//! Handing a 50-element addition to four threads loses — the dispatch costs more
//! than the arithmetic. [`should_parallelise`] therefore falls back to the
//! serial path below [`field_parallel_crossover`], whose value was **measured on
//! real hardware**, not guessed; see [`FIELD_PARALLEL_CROSSOVER`] for the table.

use std::ops::{Add, Mul, Sub};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::compute::ComputeBackend;
use crate::fields::boundary::bc::PatchField;
use crate::fields::field::Field;
use crate::fields::surface_field::SurfaceField;
use crate::fields::vol_field::{VolField, VolScalarField};
use crate::primitives::Vector3;

// ── Dispatch policy — the single place the threshold lives ───────────────────

/// Number of consecutive elements summed sequentially inside one chunk of a
/// parallel reduction.
///
/// The reductions cut the field into chunks of exactly this many elements, sum
/// each chunk in index order, then combine the partial sums in index order.
/// Fixing this at a compile-time constant — rather than deriving it from the
/// thread count — is what makes the parallel reduction **bit-reproducible and
/// thread-count independent** (see the module-level "Reduction determinism"
/// section).
///
/// Value: `4096` elements, i.e. 32 KiB of `f64`, which sits comfortably inside a
/// typical L1/L2 data cache so a chunk is summed without cache misses. The final
/// chunk is shorter when the length is not a multiple of 4096; that does not
/// affect reproducibility because the split is still a pure function of the
/// length.
pub const REDUCTION_CHUNK: usize = 4096;

/// Element count at or above which [`ComputeBackend::CpuMulti`] actually spreads
/// the work across threads. Below it, every operation runs on the calling
/// thread.
///
/// # Why a crossover exists at all
///
/// Element-wise field arithmetic is memory-bandwidth bound and rayon's dispatch
/// costs on the order of microseconds. Below some size the dispatch dominates
/// and the parallel path is strictly *slower*.
///
/// # Measured basis (not a guess)
///
/// Measured 2026-08-12, `--release --features parallel`, `f64` fields, **4
/// logical cores** reported by [`std::thread::available_parallelism`]; operation
/// `c = a + b` (out-of-place add), best of 9 repeats after two warm-ups, by the
/// `#[ignore]`d `measure_crossover_add` test in this module, run in isolation
/// (`--test-threads=1`). Absolute wall-clock per call, from the least-contended
/// of nine sweeps:
///
/// | n | serial | CpuMulti (4 threads) | speedup |
/// |---|---|---|---|
/// | 1 024 | 0.43 us | 7.59 us | 0.06x |
/// | 4 096 | 1.89 us | 11.07 us | 0.17x |
/// | 16 384 | 5.99 us | 23.11 us | 0.26x |
/// | 65 536 | 56.27 us | 30.39 us | 1.85x |
/// | 131 072 | 117.77 us | 49.87 us | 2.36x |
/// | 262 144 | 239.65 us | 89.52 us | 2.68x |
/// | 1 048 576 | 2001.64 us | 437.34 us | 4.58x |
///
/// # The crossover is a band, not a point — read this before trusting it
///
/// The measurement machine is a shared virtualised sandbox. The **serial** column
/// is highly repeatable (spread under 5% across ten sweeps); the **parallel**
/// column is not, varying by up to 4x run to run because it competes for cores
/// with whatever else the host is doing. Counting how often the parallel path
/// won, over ten independent sweeps:
///
/// | n | sweeps won |
/// |---|---|
/// | 1 024 / 4 096 / 16 384 | 0 of 10 — never |
/// | 65 536 | 4 of 10 |
/// | 131 072 | 6 of 10 |
/// | 262 144 | 9 of 10 |
/// | 1 048 576 | 10 of 10 (1.48x-4.58x) |
///
/// So the honest statement is: **the crossover lies between 65 536 and 262 144
/// elements on this hardware, and the measurement is not precise enough to pin it
/// further.** This constant is set to `131_072`, the smallest size that won a
/// majority of sweeps. The asymmetry justifies erring low: in the sweeps where
/// 131 072 lost, it lost by 3%-27% of ~120 us (tens of microseconds), whereas
/// setting the threshold at 262 144 would forfeit a measured 1.3x-2.4x on
/// mid-sized meshes.
///
/// **Re-measure on the target machine.** More cores or more memory bandwidth move
/// the crossover down; a phone moves it up. Run
/// `cargo test -p outram-foam-basic-lib --lib --release --features parallel --
/// --ignored --nocapture --test-threads=1 measure_crossover_add`.
///
/// # Relationship to [`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`]
///
/// The workspace-level threshold is `4_096` work items and its own documentation
/// states it is **a placeholder awaiting measurement**, adding that "each kernel
/// that measures its own crossover should say so in its docs and may override
/// this". This constant is that override for the field-algebra kernels, and the
/// measurement says the placeholder is far too low for them: at `n = 4 096` the
/// parallel path measured **0.17x** — about six times *slower* than serial — and
/// it did not win a single one of nine sweeps at any size below 65 536. These
/// kernels are memory-bandwidth bound with only one or two flops per element, so
/// they need far more elements to amortise dispatch than a compute-dense kernel
/// would. The measurement here is offered as input to bead `op-yvj.4.7`.
pub const FIELD_PARALLEL_CROSSOVER: usize = 131_072;

/// The element count at or above which multi-threading is worth its overhead for
/// the field-algebra kernels.
///
/// Returns [`FIELD_PARALLEL_CROSSOVER`] — the **measured** field-kernel
/// crossover, deliberately overriding the documented placeholder
/// [`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`] (see that constant's docs for
/// why the override exists and the numbers behind it).
///
/// It is a *function*, not a bare constant, so the policy can later consult a
/// runtime-configured value without touching a single operator.
///
/// **Units:** a count of field elements — cells for a volume field, internal
/// faces for a surface field. Dimensionless.
pub fn field_parallel_crossover() -> usize {
    FIELD_PARALLEL_CROSSOVER
}

/// The one place that decides Serial vs multi-threaded execution for an
/// element-wise field operation over `n` elements.
///
/// Every operator in this module routes through this function; none of them
/// contains its own size test. It returns `true` only when **all** of:
///
/// - `backend` is [`ComputeBackend::CpuMulti`] or [`ComputeBackend::Gpu`]
///   (there is no GPU field kernel yet, so `Gpu` asks for the best CPU path);
/// - [`ComputeBackend::resolve`] confirms `CpuMulti` is actually available, i.e.
///   the crate was built with the **`parallel`** feature — availability is asked
///   of [`crate::compute`], never re-implemented here;
/// - `n >= field_parallel_crossover()`.
///
/// `n` is counted in field elements — cells for a volume field, internal faces
/// for a surface field. Dimensionless.
///
/// # Relationship to [`crate::compute::select_backend`]
///
/// `select_backend` answers "which backend should I ask for?" from a work-item
/// count; this function answers "given the backend I was asked for, is this
/// particular field big enough to be worth threading?" using the **measured**
/// field-kernel crossover rather than the workspace placeholder. A caller may use
/// either; passing `select_backend(n)` straight in works and is safe, because
/// this function re-checks the size against the measured threshold.
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::fields::parallel::{field_parallel_crossover, should_parallelise};
///
/// // Serial never parallelises, whatever the size.
/// assert!(!should_parallelise(ComputeBackend::Serial, 100_000_000));
/// // Nor does any backend on a tiny field.
/// assert!(!should_parallelise(ComputeBackend::CpuMulti, 8));
/// // Above the crossover it depends only on whether `parallel` is enabled.
/// assert_eq!(
///     should_parallelise(ComputeBackend::CpuMulti, field_parallel_crossover()),
///     cfg!(feature = "parallel"),
/// );
/// ```
pub fn should_parallelise(backend: ComputeBackend, n: usize) -> bool {
    // No GPU field kernel exists yet, so a `Gpu` request means "the best CPU
    // path". Asking for `CpuMulti` here (rather than letting `resolve` keep
    // `Gpu`) also makes the answer independent of whether a GPU adapter happens
    // to be present, which is irrelevant to this module. When a GPU field kernel
    // lands it is dispatched here and no call site changes.
    let requested = match backend {
        ComputeBackend::Gpu => ComputeBackend::CpuMulti,
        other => other,
    };
    // Availability is `compute`'s question, not ours: `resolve()` degrades
    // `CpuMulti` to `Serial` when the `parallel` feature is off.
    match requested.resolve() {
        ComputeBackend::CpuMulti => n >= field_parallel_crossover(),
        ComputeBackend::Serial | ComputeBackend::Gpu => false,
    }
}

// ── Private execution helpers ────────────────────────────────────────────────
//
// Each helper has two bodies selected by the `parallel` feature, with IDENTICAL
// signatures and bounds, so the public API is byte-for-byte the same in both
// builds. The `Send + Sync` bounds are kept in the serial build too, precisely
// so enabling the feature can never change a public signature.

#[cfg(feature = "parallel")]
fn map_slice<A, U, F>(backend: ComputeBackend, a: &[A], f: F) -> Vec<U>
where
    A: Sync,
    U: Send,
    F: Fn(&A) -> U + Send + Sync,
{
    if should_parallelise(backend, a.len()) {
        a.par_iter().map(f).collect()
    } else {
        a.iter().map(f).collect()
    }
}

#[cfg(not(feature = "parallel"))]
fn map_slice<A, U, F>(_backend: ComputeBackend, a: &[A], f: F) -> Vec<U>
where
    A: Sync,
    U: Send,
    F: Fn(&A) -> U + Send + Sync,
{
    a.iter().map(f).collect()
}

#[cfg(feature = "parallel")]
fn zip_map<A, B, U, F>(backend: ComputeBackend, a: &[A], b: &[B], f: F) -> Vec<U>
where
    A: Sync,
    B: Sync,
    U: Send,
    F: Fn(&A, &B) -> U + Send + Sync,
{
    if should_parallelise(backend, a.len()) {
        a.par_iter()
            .zip(b.par_iter())
            .map(|(p, q)| f(p, q))
            .collect()
    } else {
        a.iter().zip(b.iter()).map(|(p, q)| f(p, q)).collect()
    }
}

#[cfg(not(feature = "parallel"))]
fn zip_map<A, B, U, F>(_backend: ComputeBackend, a: &[A], b: &[B], f: F) -> Vec<U>
where
    A: Sync,
    B: Sync,
    U: Send,
    F: Fn(&A, &B) -> U + Send + Sync,
{
    a.iter().zip(b.iter()).map(|(p, q)| f(p, q)).collect()
}

#[cfg(feature = "parallel")]
fn map_in_place<A, F>(backend: ComputeBackend, a: &mut [A], f: F)
where
    A: Send + Sync,
    F: Fn(&mut A) + Send + Sync,
{
    if should_parallelise(backend, a.len()) {
        a.par_iter_mut().for_each(f);
    } else {
        a.iter_mut().for_each(f);
    }
}

#[cfg(not(feature = "parallel"))]
fn map_in_place<A, F>(_backend: ComputeBackend, a: &mut [A], f: F)
where
    A: Send + Sync,
    F: Fn(&mut A) + Send + Sync,
{
    a.iter_mut().for_each(f);
}

#[cfg(feature = "parallel")]
fn zip_in_place<A, B, F>(backend: ComputeBackend, a: &mut [A], b: &[B], f: F)
where
    A: Send + Sync,
    B: Sync,
    F: Fn(&mut A, &B) + Send + Sync,
{
    if should_parallelise(backend, a.len()) {
        a.par_iter_mut()
            .zip(b.par_iter())
            .for_each(|(p, q)| f(p, q));
    } else {
        a.iter_mut().zip(b.iter()).for_each(|(p, q)| f(p, q));
    }
}

#[cfg(not(feature = "parallel"))]
fn zip_in_place<A, B, F>(_backend: ComputeBackend, a: &mut [A], b: &[B], f: F)
where
    A: Send + Sync,
    B: Sync,
    F: Fn(&mut A, &B) + Send + Sync,
{
    a.iter_mut().zip(b.iter()).for_each(|(p, q)| f(p, q));
}

/// Fixed-chunk tree sum of `f(x[i])` — see the module "Reduction determinism"
/// section. Deterministic and thread-count independent by construction.
#[cfg(feature = "parallel")]
fn tree_sum<F>(backend: ComputeBackend, x: &[f64], f: F) -> f64
where
    F: Fn(f64) -> f64 + Send + Sync,
{
    if should_parallelise(backend, x.len()) {
        let partials: Vec<f64> = x
            .par_chunks(REDUCTION_CHUNK)
            .map(|c| c.iter().map(|v| f(*v)).sum::<f64>())
            .collect();
        partials.iter().sum()
    } else {
        x.iter().map(|v| f(*v)).sum()
    }
}

#[cfg(not(feature = "parallel"))]
fn tree_sum<F>(_backend: ComputeBackend, x: &[f64], f: F) -> f64
where
    F: Fn(f64) -> f64 + Send + Sync,
{
    x.iter().map(|v| f(*v)).sum()
}

/// Fixed-chunk tree sum of `f(x[i], y[i])`. Same determinism guarantee.
#[cfg(feature = "parallel")]
fn tree_sum2<F>(backend: ComputeBackend, x: &[f64], y: &[f64], f: F) -> f64
where
    F: Fn(f64, f64) -> f64 + Send + Sync,
{
    if should_parallelise(backend, x.len()) {
        let partials: Vec<f64> = x
            .par_chunks(REDUCTION_CHUNK)
            .zip(y.par_chunks(REDUCTION_CHUNK))
            .map(|(cx, cy)| {
                cx.iter()
                    .zip(cy.iter())
                    .map(|(p, q)| f(*p, *q))
                    .sum::<f64>()
            })
            .collect();
        partials.iter().sum()
    } else {
        x.iter().zip(y.iter()).map(|(p, q)| f(*p, *q)).sum()
    }
}

#[cfg(not(feature = "parallel"))]
fn tree_sum2<F>(_backend: ComputeBackend, x: &[f64], y: &[f64], f: F) -> f64
where
    F: Fn(f64, f64) -> f64 + Send + Sync,
{
    x.iter().zip(y.iter()).map(|(p, q)| f(*p, *q)).sum()
}

/// Fixed-chunk extremum fold. `min`/`max` are associative, so this is
/// bit-identical to the serial fold.
#[cfg(feature = "parallel")]
fn tree_extremum(backend: ComputeBackend, x: &[f64], init: f64, op: fn(f64, f64) -> f64) -> f64 {
    if should_parallelise(backend, x.len()) {
        let partials: Vec<f64> = x
            .par_chunks(REDUCTION_CHUNK)
            .map(|c| c.iter().copied().fold(init, op))
            .collect();
        partials.iter().copied().fold(init, op)
    } else {
        x.iter().copied().fold(init, op)
    }
}

#[cfg(not(feature = "parallel"))]
fn tree_extremum(_backend: ComputeBackend, x: &[f64], init: f64, op: fn(f64, f64) -> f64) -> f64 {
    x.iter().copied().fold(init, op)
}

// ── Element-wise kernels on `Field<T>` ───────────────────────────────────────

/// Element-wise sum `c[i] = a[i] + b[i]`, returning a new field.
///
/// Generic over the element type: works for `Field<f64>`, `Field<Vector3>`,
/// `Field<Tensor>` and `Field<SymmTensor>`, all of which are `Copy`.
///
/// **Units:** both operands must carry the same physical quantity (this layer
/// stores bare numbers and cannot check that); the result carries it too.
///
/// Every output element is computed by the same expression as the serial
/// [`std::ops::Add`] impl on [`Field`], so the result is **bit-identical** to
/// [`ComputeBackend::Serial`] on any backend and any thread count.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`, matching the serial operator.
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::fields::field::Field;
/// use outram_foam_basic_lib::fields::parallel;
///
/// let a = Field::new(vec![1.0, 2.0, 3.0]);
/// let b = Field::new(vec![10.0, 20.0, 30.0]);
/// let c = parallel::add(ComputeBackend::CpuMulti, &a, &b);
/// assert_eq!(c.as_slice(), &[11.0, 22.0, 33.0]);
/// ```
pub fn add<T>(backend: ComputeBackend, a: &Field<T>, b: &Field<T>) -> Field<T>
where
    T: Copy + Send + Sync + Add<Output = T>,
{
    assert_eq!(a.len(), b.len(), "Field length mismatch in add");
    Field::new(zip_map(backend, a.as_slice(), b.as_slice(), |p, q| *p + *q))
}

/// Element-wise difference `c[i] = a[i] - b[i]`, returning a new field.
///
/// **Units:** as for [`add`] — both operands are the same physical quantity.
/// Bit-identical to the serial [`std::ops::Sub`] impl on [`Field`].
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
pub fn sub<T>(backend: ComputeBackend, a: &Field<T>, b: &Field<T>) -> Field<T>
where
    T: Copy + Send + Sync + Sub<Output = T>,
{
    assert_eq!(a.len(), b.len(), "Field length mismatch in sub");
    Field::new(zip_map(backend, a.as_slice(), b.as_slice(), |p, q| *p - *q))
}

/// Uniform scaling `c[i] = a[i] * s`, returning a new field.
///
/// Used for under-relaxation (`s = alpha`, dimensionless), for `1/dt` weighting
/// (`s` in `s^-1`), and for negation (`s = -1.0`).
///
/// **Units:** the result carries `[a] * [s]`; this layer treats `s` as a bare
/// number and cannot check it. Bit-identical to the serial
/// [`std::ops::Mul<f64>`] impl on [`Field`].
pub fn scale<T>(backend: ComputeBackend, a: &Field<T>, s: f64) -> Field<T>
where
    T: Copy + Send + Sync + Mul<f64, Output = T>,
{
    Field::new(map_slice(backend, a.as_slice(), |p| *p * s))
}

/// Fused combination `c[i] = y[i] + a * x[i]` ("axpy"), returning a new field.
///
/// This is the single most common shape in a solver timestep — adding a scaled
/// explicit source, applying an under-relaxed correction, or advancing an
/// explicit Euler update `phi_new = phi + dt * ddt(phi)`. It is one fused pass,
/// so the fields are traversed once instead of twice; that matters because these
/// kernels are memory-bandwidth bound.
///
/// **Units:** `y` and `a*x` must be the same physical quantity.
///
/// # Panics
///
/// Panics if `y.len() != x.len()`.
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::fields::field::Field;
/// use outram_foam_basic_lib::fields::parallel;
///
/// let y = Field::new(vec![1.0, 1.0]);
/// let x = Field::new(vec![4.0, 6.0]);
/// let c = parallel::axpy(ComputeBackend::Serial, &y, 0.5, &x);
/// assert_eq!(c.as_slice(), &[3.0, 4.0]);
/// ```
pub fn axpy<T>(backend: ComputeBackend, y: &Field<T>, a: f64, x: &Field<T>) -> Field<T>
where
    T: Copy + Send + Sync + Add<Output = T> + Mul<f64, Output = T>,
{
    assert_eq!(y.len(), x.len(), "Field length mismatch in axpy");
    Field::new(zip_map(backend, y.as_slice(), x.as_slice(), |p, q| {
        *p + *q * a
    }))
}

/// In-place accumulation `y[i] += x[i]`.
///
/// Allocation-free — the preferred form inside a timestep loop, where the
/// out-of-place [`add`] allocates a fresh `Vec` every call.
///
/// **Units:** `y` and `x` are the same physical quantity.
///
/// # Panics
///
/// Panics if `y.len() != x.len()`.
pub fn add_assign<T>(backend: ComputeBackend, y: &mut Field<T>, x: &Field<T>)
where
    T: Copy + Send + Sync + Add<Output = T>,
{
    assert_eq!(y.len(), x.len(), "Field length mismatch in add_assign");
    zip_in_place(backend, y.as_mut_slice(), x.as_slice(), |p, q| *p = *p + *q);
}

/// In-place subtraction `y[i] -= x[i]`. Allocation-free.
///
/// **Units:** `y` and `x` are the same physical quantity.
///
/// # Panics
///
/// Panics if `y.len() != x.len()`.
pub fn sub_assign<T>(backend: ComputeBackend, y: &mut Field<T>, x: &Field<T>)
where
    T: Copy + Send + Sync + Sub<Output = T>,
{
    assert_eq!(y.len(), x.len(), "Field length mismatch in sub_assign");
    zip_in_place(backend, y.as_mut_slice(), x.as_slice(), |p, q| *p = *p - *q);
}

/// In-place uniform scaling `y[i] *= s`. Allocation-free.
///
/// **Units:** the field becomes `[y] * [s]`; `s` is a bare number here.
pub fn scale_assign<T>(backend: ComputeBackend, y: &mut Field<T>, s: f64)
where
    T: Copy + Send + Sync + Mul<f64, Output = T>,
{
    map_in_place(backend, y.as_mut_slice(), |p| *p = *p * s);
}

/// In-place fused combination `y[i] += a * x[i]` ("axpy"). Allocation-free.
///
/// The hot-loop form of [`axpy`]: one traversal, no allocation. This is the
/// shape an explicit transient update takes every timestep.
///
/// **Units:** `y` and `a*x` are the same physical quantity.
///
/// # Panics
///
/// Panics if `y.len() != x.len()`.
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::fields::field::Field;
/// use outram_foam_basic_lib::fields::parallel;
/// use outram_foam_basic_lib::primitives::Vector3;
///
/// let mut u = Field::uniform(3, Vector3::new(1.0, 0.0, 0.0));   // [m/s]
/// let du = Field::uniform(3, Vector3::new(0.0, 2.0, 0.0));      // [m/s^2]
/// parallel::axpy_assign(ComputeBackend::CpuMulti, &mut u, 0.5, &du);  // dt = 0.5 s
/// assert_eq!(u[0], Vector3::new(1.0, 1.0, 0.0));
/// ```
pub fn axpy_assign<T>(backend: ComputeBackend, y: &mut Field<T>, a: f64, x: &Field<T>)
where
    T: Copy + Send + Sync + Add<Output = T> + Mul<f64, Output = T>,
{
    assert_eq!(y.len(), x.len(), "Field length mismatch in axpy_assign");
    zip_in_place(backend, y.as_mut_slice(), x.as_slice(), |p, q| {
        *p = *p + *q * a
    });
}

/// Element-wise product of two **scalar** fields, `c[i] = a[i] * b[i]`.
///
/// The workhorse behind `rho*h`, and behind any coefficient-times-field
/// assembly. **Units multiply:** the result carries `[a]*[b]`.
///
/// Bit-identical to [`Field::pointwise_mul`].
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
pub fn pointwise_mul(backend: ComputeBackend, a: &Field<f64>, b: &Field<f64>) -> Field<f64> {
    assert_eq!(a.len(), b.len(), "Field length mismatch in pointwise_mul");
    Field::new(zip_map(backend, a.as_slice(), b.as_slice(), |p, q| p * q))
}

/// Element-wise quotient of two **scalar** fields, `c[i] = a[i] / b[i]`.
///
/// **Units divide:** the result carries `[a]/[b]`. Division by zero yields
/// `+/-inf` or `NaN` exactly as `f64` division does — no guard is applied,
/// matching [`Field::pointwise_div`].
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
pub fn pointwise_div(backend: ComputeBackend, a: &Field<f64>, b: &Field<f64>) -> Field<f64> {
    assert_eq!(a.len(), b.len(), "Field length mismatch in pointwise_div");
    Field::new(zip_map(backend, a.as_slice(), b.as_slice(), |p, q| p / q))
}

/// Scale a vector/tensor field by a per-element scalar field:
/// `c[i] = v[i] * s[i]`.
///
/// This is `rho*U` (density field times velocity field) and every other
/// "coefficient field times ranked field" product. **Units multiply:**
/// `[v]*[s]`.
///
/// Bit-identical to [`Field::scale`] for `Field<Vector3>`.
///
/// # Panics
///
/// Panics if `v.len() != s.len()`.
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::fields::field::Field;
/// use outram_foam_basic_lib::fields::parallel;
/// use outram_foam_basic_lib::primitives::Vector3;
///
/// let u = Field::uniform(2, Vector3::new(1.0, 2.0, 3.0));   // [m/s]
/// let rho = Field::new(vec![2.0, 0.5]);                     // [kg/m^3]
/// let rho_u = parallel::scale_by_field(ComputeBackend::CpuMulti, &u, &rho); // [kg/(m^2 s)]
/// assert_eq!(rho_u[0], Vector3::new(2.0, 4.0, 6.0));
/// assert_eq!(rho_u[1], Vector3::new(0.5, 1.0, 1.5));
/// ```
pub fn scale_by_field<T>(backend: ComputeBackend, v: &Field<T>, s: &Field<f64>) -> Field<T>
where
    T: Copy + Send + Sync + Mul<f64, Output = T>,
{
    assert_eq!(v.len(), s.len(), "Field length mismatch in scale_by_field");
    Field::new(zip_map(backend, v.as_slice(), s.as_slice(), |p, q| *p * *q))
}

/// Element-wise dot product of two vector fields → scalar field:
/// `c[i] = a[i] . b[i]`.
///
/// **Units multiply:** `[a]*[b]`. Bit-identical to [`Field::dot_field`].
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
pub fn dot_field(backend: ComputeBackend, a: &Field<Vector3>, b: &Field<Vector3>) -> Field<f64> {
    assert_eq!(a.len(), b.len(), "Field length mismatch in dot_field");
    Field::new(zip_map(backend, a.as_slice(), b.as_slice(), |p, q| {
        p.dot(*q)
    }))
}

// ── Reductions on `Field<f64>` ───────────────────────────────────────────────

/// Sum of all elements, `sum_i x[i]`.
///
/// **Units:** the field's own units. An empty field sums to `0.0`.
///
/// # Determinism
///
/// Reproducible run to run and independent of thread count, but **not**
/// bit-identical to the serial [`Field::sum`] — the summation order differs. See
/// the module-level "Reduction determinism" section for the measured deviation.
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::fields::field::Field;
/// use outram_foam_basic_lib::fields::parallel;
///
/// assert_eq!(parallel::sum(ComputeBackend::Serial, &Field::new(vec![1.0, 2.0, 3.0])), 6.0);
/// assert_eq!(parallel::sum(ComputeBackend::CpuMulti, &Field::<f64>::new(vec![])), 0.0);
/// ```
pub fn sum(backend: ComputeBackend, x: &Field<f64>) -> f64 {
    tree_sum(backend, x.as_slice(), |v| v)
}

/// Arithmetic mean of all elements; returns `0.0` for an empty field, matching
/// [`Field::mean`].
///
/// **Units:** the field's own units.
pub fn mean(backend: ComputeBackend, x: &Field<f64>) -> f64 {
    if x.is_empty() {
        0.0
    } else {
        sum(backend, x) / x.len() as f64
    }
}

/// Smallest element. Returns `+inf` for an empty field, matching [`Field::min`].
///
/// **Units:** the field's own units. **Bit-identical** to the serial fold on
/// every backend, because `min` is associative.
pub fn min(backend: ComputeBackend, x: &Field<f64>) -> f64 {
    tree_extremum(backend, x.as_slice(), f64::INFINITY, f64::min)
}

/// Largest element. Returns `-inf` for an empty field, matching [`Field::max`].
///
/// **Units:** the field's own units. **Bit-identical** to the serial fold on
/// every backend, because `max` is associative.
pub fn max(backend: ComputeBackend, x: &Field<f64>) -> f64 {
    tree_extremum(backend, x.as_slice(), f64::NEG_INFINITY, f64::max)
}

/// Euclidean (L2) norm, `sqrt(sum_i x[i]^2)`.
///
/// This is the convergence measure a solver evaluates on the residual field
/// every outer iteration. **Units:** the field's own units. An empty field gives
/// `0.0`.
///
/// # Determinism
///
/// Same guarantee as [`sum`]: reproducible and thread-count independent, not
/// bit-identical to [`Field::l2_norm`].
pub fn l2_norm(backend: ComputeBackend, x: &Field<f64>) -> f64 {
    tree_sum(backend, x.as_slice(), |v| v * v).sqrt()
}

/// Inner product of two scalar fields, `sum_i a[i]*b[i]`.
///
/// The Krylov-solver inner product, and — with cell volumes as `b` — the
/// volume-weighted integral. **Units multiply:** `[a]*[b]`.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
///
/// # Determinism
///
/// Same guarantee as [`sum`].
pub fn dot(backend: ComputeBackend, a: &Field<f64>, b: &Field<f64>) -> f64 {
    assert_eq!(a.len(), b.len(), "Field length mismatch in dot");
    tree_sum2(backend, a.as_slice(), b.as_slice(), |p, q| p * q)
}

// ── `VolField` / `SurfaceField` wrappers ─────────────────────────────────────
//
// Every one of these copies the LEFT operand's `name` verbatim. Never compose a
// name here — see the module docs and the crate `CLAUDE.md`.

/// Apply `op` to each patch pair, keeping the left patch's boundary condition.
fn zip_patches<T, F>(lhs: &[PatchField<T>], rhs: &[PatchField<T>], op: F) -> Vec<PatchField<T>>
where
    T: Clone,
    F: Fn(&Field<T>, &Field<T>) -> Field<T>,
{
    lhs.iter()
        .zip(rhs.iter())
        .map(|(l, r)| PatchField {
            bc: l.bc.clone(),
            values: op(&l.values, &r.values),
        })
        .collect()
}

/// Apply `op` to each patch, keeping the boundary condition.
fn map_patches<T, F>(patches: &[PatchField<T>], op: F) -> Vec<PatchField<T>>
where
    T: Clone,
    F: Fn(&Field<T>) -> Field<T>,
{
    patches
        .iter()
        .map(|p| PatchField {
            bc: p.bc.clone(),
            values: op(&p.values),
        })
        .collect()
}

/// Sum of two volume fields — internal field **and** every boundary patch.
///
/// The result takes `a`'s `name`, `a`'s mesh, and `a`'s per-patch boundary
/// conditions, exactly like the serial [`std::ops::Add`] impl on [`VolField`].
///
/// **The name is copied, never composed.** See the module docs: composing names
/// here produces `2^step` string growth in a solver loop.
///
/// **Units:** both operands are the same physical quantity.
///
/// # Panics
///
/// Panics if the internal fields or any patch pair differ in length.
///
/// ```rust
/// use std::sync::Arc;
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::fields::parallel;
/// use outram_foam_basic_lib::fields::vol_field::VolScalarField;
/// use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
///
/// let mesh = Arc::new(FvMesh::periodic_1d(8, 1.0, 1.0));
/// let a = VolScalarField::uniform("rho", mesh.clone(), 1.0);
/// let b = VolScalarField::uniform("drho", mesh, 0.25);
/// let c = parallel::add_vol(ComputeBackend::CpuMulti, &a, &b);
/// assert_eq!(c.name, "rho");                 // NOT "(rho + drho)"
/// assert!((c.internal[0] - 1.25).abs() < 1e-15);
/// ```
pub fn add_vol<T>(backend: ComputeBackend, a: &VolField<T>, b: &VolField<T>) -> VolField<T>
where
    T: Copy + Default + Send + Sync + Add<Output = T>,
{
    let internal = add(backend, &a.internal, &b.internal);
    let boundary = zip_patches(&a.boundary, &b.boundary, |l, r| add(backend, l, r));
    VolField::new(a.name.clone(), a.mesh.clone(), internal, boundary)
}

/// Difference of two volume fields — internal field and every boundary patch.
///
/// Name, mesh, and boundary conditions come from `a`; the name is copied, never
/// composed. **Units:** both operands are the same physical quantity.
///
/// # Panics
///
/// Panics if the internal fields or any patch pair differ in length.
pub fn sub_vol<T>(backend: ComputeBackend, a: &VolField<T>, b: &VolField<T>) -> VolField<T>
where
    T: Copy + Default + Send + Sync + Sub<Output = T>,
{
    let internal = sub(backend, &a.internal, &b.internal);
    let boundary = zip_patches(&a.boundary, &b.boundary, |l, r| sub(backend, l, r));
    VolField::new(a.name.clone(), a.mesh.clone(), internal, boundary)
}

/// Uniform scaling of a volume field, `c = a * s` — internal field and every
/// boundary patch.
///
/// Name, mesh, and boundary conditions come from `a`. **Units:** `[a]*[s]`.
pub fn scale_vol<T>(backend: ComputeBackend, a: &VolField<T>, s: f64) -> VolField<T>
where
    T: Copy + Default + Send + Sync + Mul<f64, Output = T>,
{
    let internal = scale(backend, &a.internal, s);
    let boundary = map_patches(&a.boundary, |v| scale(backend, v, s));
    VolField::new(a.name.clone(), a.mesh.clone(), internal, boundary)
}

/// In-place accumulation on a volume field, `y += x` — internal field and every
/// boundary patch.
///
/// Allocation-free and **name-preserving by construction**: `y.name` is never
/// touched. **Units:** `y` and `x` are the same physical quantity.
///
/// # Panics
///
/// Panics if the internal fields or any patch pair differ in length.
pub fn add_vol_assign<T>(backend: ComputeBackend, y: &mut VolField<T>, x: &VolField<T>)
where
    T: Copy + Send + Sync + Add<Output = T>,
{
    add_assign(backend, &mut y.internal, &x.internal);
    for (l, r) in y.boundary.iter_mut().zip(x.boundary.iter()) {
        add_assign(backend, &mut l.values, &r.values);
    }
}

/// In-place subtraction on a volume field, `y -= x`. Allocation-free,
/// name-preserving.
///
/// **Units:** `y` and `x` are the same physical quantity.
///
/// # Panics
///
/// Panics if the internal fields or any patch pair differ in length.
pub fn sub_vol_assign<T>(backend: ComputeBackend, y: &mut VolField<T>, x: &VolField<T>)
where
    T: Copy + Send + Sync + Sub<Output = T>,
{
    sub_assign(backend, &mut y.internal, &x.internal);
    for (l, r) in y.boundary.iter_mut().zip(x.boundary.iter()) {
        sub_assign(backend, &mut l.values, &r.values);
    }
}

/// In-place uniform scaling of a volume field, `y *= s`. Allocation-free,
/// name-preserving. **Units:** the field becomes `[y]*[s]`.
pub fn scale_vol_assign<T>(backend: ComputeBackend, y: &mut VolField<T>, s: f64)
where
    T: Copy + Send + Sync + Mul<f64, Output = T>,
{
    scale_assign(backend, &mut y.internal, s);
    for p in y.boundary.iter_mut() {
        scale_assign(backend, &mut p.values, s);
    }
}

/// In-place fused update on a volume field, `y += a * x` — internal field and
/// every boundary patch.
///
/// This is the explicit-update shape a transient solver runs on every prognostic
/// field every timestep (`rho += dt * ddt(rho)`). Allocation-free and
/// name-preserving — the form that structurally cannot reproduce the
/// `name`-growth bug, because it never constructs a new field.
///
/// **Units:** `y` and `a*x` are the same physical quantity.
///
/// # Panics
///
/// Panics if the internal fields or any patch pair differ in length.
///
/// ```rust
/// use std::sync::Arc;
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::fields::parallel;
/// use outram_foam_basic_lib::fields::vol_field::VolScalarField;
/// use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
///
/// let mesh = Arc::new(FvMesh::periodic_1d(16, 1.0, 1.0));
/// let mut rho = VolScalarField::uniform("rho", mesh.clone(), 1.0);
/// let ddt = VolScalarField::uniform("ddt(rho)", mesh, 2.0);
///
/// for _ in 0..10 {
///     parallel::axpy_vol_assign(ComputeBackend::CpuMulti, &mut rho, 0.1, &ddt); // dt = 0.1
/// }
/// assert!((rho.internal[0] - 3.0).abs() < 1e-12);
/// assert_eq!(rho.name, "rho");        // still 3 characters after 10 steps
/// ```
pub fn axpy_vol_assign<T>(backend: ComputeBackend, y: &mut VolField<T>, a: f64, x: &VolField<T>)
where
    T: Copy + Send + Sync + Add<Output = T> + Mul<f64, Output = T>,
{
    axpy_assign(backend, &mut y.internal, a, &x.internal);
    for (l, r) in y.boundary.iter_mut().zip(x.boundary.iter()) {
        axpy_assign(backend, &mut l.values, a, &r.values);
    }
}

/// Sum of two surface fields — internal faces and every boundary patch.
///
/// Name, mesh, and boundary conditions come from `a`; the name is copied, never
/// composed. **Units:** both operands are the same physical quantity.
///
/// # Panics
///
/// Panics if the internal fields or any patch pair differ in length.
pub fn add_surface<T>(
    backend: ComputeBackend,
    a: &SurfaceField<T>,
    b: &SurfaceField<T>,
) -> SurfaceField<T>
where
    T: Copy + Send + Sync + Add<Output = T>,
{
    let internal = add(backend, &a.internal, &b.internal);
    let boundary = zip_patches(&a.boundary, &b.boundary, |l, r| add(backend, l, r));
    SurfaceField::new(a.name.clone(), a.mesh.clone(), internal, boundary)
}

/// Difference of two surface fields — internal faces and every boundary patch.
///
/// Name, mesh, and boundary conditions come from `a`. **Units:** both operands
/// are the same physical quantity.
///
/// # Panics
///
/// Panics if the internal fields or any patch pair differ in length.
pub fn sub_surface<T>(
    backend: ComputeBackend,
    a: &SurfaceField<T>,
    b: &SurfaceField<T>,
) -> SurfaceField<T>
where
    T: Copy + Send + Sync + Sub<Output = T>,
{
    let internal = sub(backend, &a.internal, &b.internal);
    let boundary = zip_patches(&a.boundary, &b.boundary, |l, r| sub(backend, l, r));
    SurfaceField::new(a.name.clone(), a.mesh.clone(), internal, boundary)
}

/// Uniform scaling of a surface field, `c = a * s` — internal faces and every
/// boundary patch.
///
/// Name, mesh, and boundary conditions come from `a`. **Units:** `[a]*[s]`.
pub fn scale_surface<T>(backend: ComputeBackend, a: &SurfaceField<T>, s: f64) -> SurfaceField<T>
where
    T: Copy + Send + Sync + Mul<f64, Output = T>,
{
    let internal = scale(backend, &a.internal, s);
    let boundary = map_patches(&a.boundary, |v| scale(backend, v, s));
    SurfaceField::new(a.name.clone(), a.mesh.clone(), internal, boundary)
}

/// In-place fused update on a surface field, `y += a * x` — the flux-field
/// counterpart of [`axpy_vol_assign`] (`phi += dt * ddt(phi)`).
///
/// Allocation-free, name-preserving. **Units:** `y` and `a*x` are the same
/// physical quantity.
///
/// # Panics
///
/// Panics if the internal fields or any patch pair differ in length.
pub fn axpy_surface_assign<T>(
    backend: ComputeBackend,
    y: &mut SurfaceField<T>,
    a: f64,
    x: &SurfaceField<T>,
) where
    T: Copy + Send + Sync + Add<Output = T> + Mul<f64, Output = T>,
{
    axpy_assign(backend, &mut y.internal, a, &x.internal);
    for (l, r) in y.boundary.iter_mut().zip(x.boundary.iter()) {
        axpy_assign(backend, &mut l.values, a, &r.values);
    }
}

// ── Mesh-weighted reductions on `VolScalarField` ─────────────────────────────

/// Volume integral over the **internal** cells,
/// `integral(phi dV) = sum_i phi[i] * V[i]`.
///
/// `V[i]` is the cell volume in `m^3`, taken from the field's own mesh, so the
/// result carries **units `[phi] * m^3`** — for a density field in `kg/m^3` this
/// is the total mass in `kg`, which is the conservation check a compressible
/// solver runs each timestep.
///
/// Boundary patches are **not** included: they carry face values, not cell
/// values, and have no volume. An empty mesh integrates to `0.0`.
///
/// # Panics
///
/// Panics if the internal field length differs from `mesh.n_cells`.
///
/// # Determinism
///
/// Same guarantee as [`sum`]: reproducible run to run and independent of thread
/// count, not bit-identical to a serial left fold.
///
/// ```rust
/// use std::sync::Arc;
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::fields::parallel;
/// use outram_foam_basic_lib::fields::vol_field::VolScalarField;
/// use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
///
/// // 10 cells over a 1 m length, 1 m^2 area => total volume 1 m^3.
/// let mesh = Arc::new(FvMesh::periodic_1d(10, 1.0, 1.0));
/// let rho = VolScalarField::uniform("rho", mesh, 1.2);   // [kg/m^3]
/// let mass = parallel::vol_integral(ComputeBackend::CpuMulti, &rho);
/// assert!((mass - 1.2).abs() < 1e-12);                   // 1.2 kg
/// ```
pub fn vol_integral(backend: ComputeBackend, phi: &VolScalarField) -> f64 {
    let v = phi.mesh.cell_volumes.as_slice();
    let x = phi.internal.as_slice();
    assert_eq!(
        x.len(),
        v.len(),
        "VolScalarField internal length must equal mesh.n_cells for vol_integral"
    );
    tree_sum2(backend, x, v, |p, q| p * q)
}

/// Volume-weighted average over the internal cells,
/// `integral(phi dV) / integral(dV)`.
///
/// **Units:** the field's own units (the `m^3` cancels). Returns `0.0` when the
/// total volume is zero (an empty mesh) rather than `NaN`.
///
/// # Determinism
///
/// Same guarantee as [`sum`].
pub fn vol_average(backend: ComputeBackend, phi: &VolScalarField) -> f64 {
    let total = tree_sum(backend, phi.mesh.cell_volumes.as_slice(), |q| q);
    if total == 0.0 {
        0.0
    } else {
        vol_integral(backend, phi) / total
    }
}

/// L2 norm of a volume field's **internal** values — the residual measure a
/// solver's outer loop tests for convergence.
///
/// **Units:** the field's own units. Boundary patches are excluded.
///
/// # Determinism
///
/// Same guarantee as [`sum`].
pub fn vol_l2_norm(backend: ComputeBackend, phi: &VolScalarField) -> f64 {
    l2_norm(backend, &phi.internal)
}

/// Smallest internal value of a volume field (`+inf` on an empty mesh).
///
/// **Units:** the field's own units. Bit-identical to a serial fold.
pub fn vol_min(backend: ComputeBackend, phi: &VolScalarField) -> f64 {
    min(backend, &phi.internal)
}

/// Largest internal value of a volume field (`-inf` on an empty mesh).
///
/// **Units:** the field's own units. Bit-identical to a serial fold.
pub fn vol_max(backend: ComputeBackend, phi: &VolScalarField) -> f64 {
    max(backend, &phi.internal)
}

#[cfg(test)]
mod tests;
