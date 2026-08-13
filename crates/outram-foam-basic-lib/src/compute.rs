// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Where a numerical kernel runs — serial, multi-CPU, or GPU.
//!
//! # What belongs in this module
//!
//! The **dispatch layer and nothing else**: the [`ComputeBackend`] enum, the
//! worker-count policy [`ThreadCount`], the named backend-selection policy
//! [`select_backend`], and the GPU adapter probe. It is deliberately free of
//! numerical code so that every kernel in the crate answers the question
//! "where do I run?" the same way.
//!
//! # What does NOT belong here
//!
//! Kernels. A root finder, a sparse matrix-vector product, a quadrature rule
//! or an ODE ensemble lives in its own module (`polynomial/`, `ldu_matrix/`,
//! `krylov/`, `math/`, `ode/`) and takes a [`ComputeBackend`] as a parameter.
//! If you find yourself writing physics or arithmetic in this file, it is in
//! the wrong file.
//!
//! # Hybrid means dispatch, not two APIs
//!
//! A kernel exposes **one** public entry point with the backend as a
//! parameter. It does not grow a `foo_parallel()` sibling beside `foo()`.
//! Callers that do not care pass [`ComputeBackend::Auto`]-style resolution via
//! [`select_backend`]; callers that do care name the backend explicitly and
//! get exactly it, or a documented fallback.
//!
//! # The serial path is the oracle
//!
//! [`ComputeBackend::Serial`] is always compiled, on every target, under every
//! feature combination, and is the `#[default]`. It is the reference
//! implementation that the other two are checked against, and it is the only
//! one guaranteed to be bit-for-bit reproducible run to run. Verification and
//! validation are judged against it. The other backends are *acceleration*.
//!
//! # Cargo features — both OFF by default
//!
//! | Feature | Adds | Backend enabled |
//! |---|---|---|
//! | *(none)* | — | [`ComputeBackend::Serial`] |
//! | `parallel` | `rayon` | [`ComputeBackend::CpuMulti`] |
//! | `gpu` | `wgpu` (non-Android only) | [`ComputeBackend::Gpu`] |
//!
//! A default build pulls neither dependency, which keeps the crate
//! dependency-light and Android/Termux clean. Requesting a backend whose
//! feature is off is **not an error**: it resolves down to the best available
//! one (see [`ComputeBackend::resolve`]). That way a caller written on a
//! desktop with `--features gpu` still runs, correctly and more slowly, on a
//! phone with no features at all.
//!
//! # Android / Termux
//!
//! `rayon` is pure Rust with no system component, so the `parallel` feature is
//! **not** target-gated — it works on Android. `wgpu` is target-gated off
//! Android in `Cargo.toml`, because Android has no system Vulkan/Metal loader
//! and the workspace Android rule forbids GPU dependencies in a library build.
//! Enabling `gpu` on an Android target therefore yields a build with no GPU
//! backend rather than a broken one.
//!
//! # Planned: hybrid CPU+GPU co-execution (NOT implemented)
//!
//! There is no `Hybrid` variant and no GPU kernel in this crate — see
//! [`gpu_adapter_present`] for the extent of the `wgpu` use. One policy
//! decision for it is nevertheless already settled, because it constrains how
//! the kernels are written.
//!
//! For the **iterative** kernels the shape is *coarse-to-fine*, not a split
//! batch: a GPU `f32` pass produces only the **initial guess**, and the CPU
//! converges it to the final answer at `f64`. The result is therefore entirely
//! `f64` and the `f32` accuracy floors govern the guess, not the answer. This
//! suits batched root finding especially well — [`RootProblem::guess`] already
//! guarantees that a bad guess is a performance problem and not a correctness
//! one, and Newton's quadratic convergence turns an `f32`-accurate start into
//! a `f64` answer in about two iterations.
//!
//! [`RootProblem::guess`]: crate::math::parallel::RootProblem::guess
//!
//! For the **elementwise** kernels, where a batch genuinely is split across the
//! two devices, the CPU half runs `f32` to match the GPU, so that precision is
//! uniform across one output array rather than varying with the split ratio.
//! Those kernels are an `f32`-class throughput path sitting roughly `1e-7`
//! relative from the [`ComputeBackend::Serial`] oracle.
//!
//! Numerical differentiation is excluded from both shapes: a finite difference
//! has no fixed point to converge to, so there is no guess to seed and the
//! `f32` floor would land directly on the answer.
//!
//! Full reasoning, the per-kernel floors at `f32`, the measured iteration
//! savings, and the caveats none of this resolves are in
//! `docs/hybrid-precision-policy.md`.
//!
//! # Units
//!
//! Nothing in this module carries a physical dimension. Sizes are counts of
//! independent work items, which is a pure number. `uom` typing belongs on the
//! kernels' own signatures and must not be stripped to get data onto a device —
//! convert at the buffer boundary and convert back.

use std::sync::OnceLock;

/// Which execution backend a numerical kernel should use.
///
/// # Variants and what they cost
///
/// - [`Serial`](Self::Serial) — one thread, scalar. Always available. The
///   deterministic trusted reference; bit-for-bit reproducible run to run.
/// - [`CpuMulti`](Self::CpuMulti) — `rayon` across CPU cores. Requires the
///   `parallel` feature. Pays thread-pool overhead, so it loses on small
///   problems; see [`CPU_MULTI_MIN_WORK_ITEMS`]. Reduction order is not
///   generally reproducible unless a kernel says otherwise.
/// - [`Gpu`](Self::Gpu) — `wgpu` compute. Requires the `gpu` feature, a
///   non-Android target, and an actual adapter at run time. Pays a host-device
///   transfer per dispatch, so it needs a much larger problem to win; see
///   [`GPU_MIN_WORK_ITEMS`]. WGSL has no `f64`, so a GPU kernel that computes
///   in `f32` will NOT match the serial `f64` path to full precision — each
///   kernel must document its own measured deviation.
///
/// # Choosing one
///
/// Prefer [`select_backend`] over hand-picking. If you do name a backend,
/// route it through [`resolve`](Self::resolve) so an unavailable choice
/// degrades instead of failing.
///
/// # Units
///
/// Dimensionless — this is a mode selector, not a quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ComputeBackend {
    /// Single-threaded scalar execution — always present, always the oracle.
    #[default]
    Serial,
    /// Multi-threaded CPU execution via `rayon` (`parallel` feature).
    CpuMulti,
    /// GPU compute via `wgpu` (`gpu` feature, non-Android, adapter present).
    Gpu,
}

/// The number of work items below which [`ComputeBackend::CpuMulti`] is not
/// worth its thread-pool overhead, and [`select_backend`] returns
/// [`ComputeBackend::Serial`] instead.
///
/// # How this number was chosen
///
/// **It is a placeholder, it has NOT been justified by measurement, and the
/// one kernel family that has since been measured found it far too low.**
/// Bead `op-yvj.4.7` covers the crossover benchmarks that will replace it.
///
/// The failure mode this guards is real and easy to reproduce: spawning
/// threads to add two 50-element vectors is slower than doing it serially.
///
/// # Measured counter-evidence — read before relying on this
///
/// [`crate::fields::parallel`] measured its own crossover on 2026-08-12 (4
/// logical cores, release, `--features parallel`) and found that **at exactly
/// this threshold the parallel path was about 6x SLOWER** — `0.17x` speedup at
/// `n = 4096`, and it lost 10 sweeps out of 10 at every size below 65 536.
/// Those kernels are memory-bandwidth bound at 1-2 flops per element, so
/// thread overhead dominates much further out than it would for compute-dense
/// work. That module therefore overrides this constant at 131 072 via
/// [`crate::fields::parallel::field_parallel_crossover`].
///
/// [`crate::ldu_matrix::parallel`] measured the same day and found the
/// opposite for *its* sparse matrix-vector product: 4096 cells is precisely
/// where that kernel breaks even, so `SPMV_MIN_CELLS` keeps this value. But
/// the vector operations in that same module want `VECOP_MIN_ELEMENTS =
/// 262 144`, because `axpy` at 4096 elements runs at `0.05x` — twenty times
/// slower.
///
/// [`crate::math::parallel`] then measured the opposite direction:
/// batched root finding crosses over at **256** problems, 16x *below* this
/// constant, because it is compute-dense per lane where the field kernels are
/// memory-bound.
///
/// Four kernel families have now been measured and they want **256, 4 096,
/// 131 072 and 262 144** — a 1024x spread, with this constant sitting in the
/// middle and wrong at both ends. That is the real finding: no single
/// crate-wide threshold can be right for all of them.
///
/// Worse for any hope of a universal number: the root-finding crossover
/// depends on the *caller's* residual cost, which this crate cannot know. A
/// cheap residual and an expensive one cross over at different sizes with
/// identical kernel code.
///
/// The honest reading: this constant is a *floor against absurdity*, not a
/// tuned value, and a kernel that has not measured its own crossover should
/// not assume this one is safe for it. Per-kernel overrides are expected, not
/// exceptional.
///
/// # Units
///
/// A count of independent work items (cells, faces, roots, quadrature points),
/// dimensionless.
pub const CPU_MULTI_MIN_WORK_ITEMS: usize = 4_096;

/// The number of work items below which [`ComputeBackend::Gpu`] is not worth
/// the host-device transfer, and [`select_backend`] falls back to the best
/// available CPU backend.
///
/// # How this number was chosen
///
/// **Placeholder awaiting measurement — see [`CPU_MULTI_MIN_WORK_ITEMS`].**
/// It is set an order of magnitude above the CPU-multi threshold because a GPU
/// dispatch pays for a round trip across the PCIe bus in addition to kernel
/// launch, so it needs correspondingly more work to amortise.
///
/// Note that this workspace's own experience argues for caution rather than
/// optimism about GPU wins: beads `op-u6s.8` and `op-u6s.9` record GPU
/// transport work struggling to beat a good multi-threaded CPU path, and
/// `op-u6s.9` is still open.
///
/// # Units
///
/// A count of independent work items, dimensionless.
pub const GPU_MIN_WORK_ITEMS: usize = 65_536;

impl ComputeBackend {
    /// Whether this backend can actually run in this build, on this target,
    /// right now.
    ///
    /// - [`Serial`](Self::Serial) — always `true`.
    /// - [`CpuMulti`](Self::CpuMulti) — `true` when the `parallel` feature is
    ///   on. Compile-time only; rayon needs no run-time probe.
    /// - [`Gpu`](Self::Gpu) — `true` only when the `gpu` feature is on, the
    ///   target is not Android, **and** a GPU adapter was found by
    ///   [`gpu_adapter_present`]. This one genuinely queries the machine.
    #[must_use]
    pub fn is_available(self) -> bool {
        match self {
            Self::Serial => true,
            Self::CpuMulti => cfg!(feature = "parallel"),
            Self::Gpu => gpu_adapter_present(),
        }
    }

    /// Degrade this backend to the best one that is actually available,
    /// without erroring.
    ///
    /// The order is `Gpu -> CpuMulti -> Serial`: a requested GPU with no
    /// adapter (or no `gpu` feature) tries multi-CPU, and failing that runs
    /// serially. [`Serial`](Self::Serial) is always reachable, so this never
    /// fails and never panics.
    ///
    /// This is the "graceful absence of a GPU" contract: a missing adapter is
    /// a normal outcome on a headless server, in CI, on Android, or on a
    /// laptop with no discrete GPU — not an error condition.
    #[must_use]
    pub fn resolve(self) -> Self {
        match self {
            Self::Gpu if Self::Gpu.is_available() => Self::Gpu,
            Self::Gpu | Self::CpuMulti if Self::CpuMulti.is_available() => Self::CpuMulti,
            _ => Self::Serial,
        }
    }

    /// A short human-readable label, for benchmark tables and log lines.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Serial => "serial",
            Self::CpuMulti => "cpu-multi",
            Self::Gpu => "gpu",
        }
    }
}

/// **The** backend-selection policy — one named function, so a reader can find
/// the rule instead of hunting for scattered `if n > 1024` tests.
///
/// # The rule
///
/// Given `work_items`, the number of *independent* items a kernel will process:
///
/// 1. If `work_items >= ` [`GPU_MIN_WORK_ITEMS`] and the GPU is available, use
///    [`ComputeBackend::Gpu`].
/// 2. Otherwise if `work_items >= ` [`CPU_MULTI_MIN_WORK_ITEMS`] and the
///    `parallel` feature is on, use [`ComputeBackend::CpuMulti`].
/// 3. Otherwise use [`ComputeBackend::Serial`].
///
/// # Arguments
///
/// - `work_items` — count of independent work items (cells, faces, roots,
///   quadrature points). Dimensionless. Pass the honest count: passing a
///   matrix's row count when the kernel actually loops over its faces will
///   pick the wrong backend.
///
/// # Returns
///
/// A backend that is guaranteed available, so the caller may use it directly
/// without a further [`ComputeBackend::resolve`].
///
/// # Why a caller might not use this
///
/// Benchmarks and parity gates need to run a *named* backend regardless of
/// size — that is what [`ComputeBackend::resolve`] is for. This function is
/// for production callers that just want the fastest correct option.
///
/// # Example
///
/// ```
/// use outram_foam_basic_lib::compute::{select_backend, ComputeBackend};
///
/// // A tiny problem always runs serially, whatever features are enabled.
/// assert_eq!(select_backend(16), ComputeBackend::Serial);
///
/// // Whatever it picks for a large problem is guaranteed to be runnable.
/// assert!(select_backend(1_000_000).is_available());
/// ```
#[must_use]
pub fn select_backend(work_items: usize) -> ComputeBackend {
    if work_items >= GPU_MIN_WORK_ITEMS && ComputeBackend::Gpu.is_available() {
        ComputeBackend::Gpu
    } else if work_items >= CPU_MULTI_MIN_WORK_ITEMS && ComputeBackend::CpuMulti.is_available() {
        ComputeBackend::CpuMulti
    } else {
        ComputeBackend::Serial
    }
}

/// How many worker threads [`ComputeBackend::CpuMulti`] should use.
///
/// Mirrors the established shape in `boon-lay`'s and `outram-mc-libs`'s own
/// compute modules so the workspace has one vocabulary for this rather than
/// three.
///
/// # Units
///
/// Dimensionless counts. [`Fraction`](Self::Fraction) is a pure ratio.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ThreadCount {
    /// Every logical core, via [`std::thread::available_parallelism`], falling
    /// back to 1 if the query fails. The default.
    #[default]
    Auto,
    /// An explicit worker count, clamped up to a minimum of 1.
    Fixed(usize),
    /// A fraction of the available logical cores, e.g. `0.5` for half. The
    /// product is rounded to nearest and clamped to at least 1. A non-finite
    /// or non-positive fraction clamps to 1 rather than panicking.
    Fraction(f64),
}

impl ThreadCount {
    /// Resolve to a concrete worker-thread count, always `>= 1`.
    ///
    /// # Returns
    ///
    /// A thread count in `[1, usize::MAX]`. Never zero, so it is always safe
    /// to hand to a thread-pool builder.
    ///
    /// # Example
    ///
    /// ```
    /// use outram_foam_basic_lib::compute::ThreadCount;
    ///
    /// assert_eq!(ThreadCount::Fixed(0).resolve(), 1); // clamped, not a panic
    /// assert_eq!(ThreadCount::Fixed(3).resolve(), 3);
    /// assert!(ThreadCount::Auto.resolve() >= 1);
    /// ```
    #[must_use]
    pub fn resolve(self) -> usize {
        let cores = || {
            std::thread::available_parallelism()
                .map(std::num::NonZeroUsize::get)
                .unwrap_or(1)
        };
        match self {
            Self::Auto => cores(),
            Self::Fixed(n) => n.max(1),
            Self::Fraction(f) => {
                if !f.is_finite() || f <= 0.0 {
                    1
                } else {
                    ((cores() as f64) * f).round().max(1.0) as usize
                }
            }
        }
    }
}

/// Cached answer to "is there a usable GPU adapter on this machine?", so the
/// probe runs at most once per process.
static GPU_PRESENT: OnceLock<bool> = OnceLock::new();

/// Whether a usable GPU compute adapter exists, probed once and cached.
///
/// # Behaviour
///
/// - Without the `gpu` feature, or on Android, this is `false` with no probe
///   attempted.
/// - Otherwise it asks `wgpu` for an adapter, once. `false` is a normal
///   outcome — headless CI, a container, a machine with no discrete GPU — and
///   is **not** an error. Nothing here panics or returns `Err`.
///
/// # Returns
///
/// `true` only if a GPU kernel could actually be dispatched.
#[must_use]
pub fn gpu_adapter_present() -> bool {
    *GPU_PRESENT.get_or_init(probe_gpu_adapter)
}

/// The one-shot probe behind [`gpu_adapter_present`] — Android and
/// feature-disabled builds.
#[cfg(any(not(feature = "gpu"), target_os = "android"))]
fn probe_gpu_adapter() -> bool {
    false
}

/// The one-shot probe behind [`gpu_adapter_present`] — desktop builds with the
/// `gpu` feature.
///
/// Requests a headless adapter (no surface, so no display handle is needed).
/// A failure is reported as `false`, never as a panic.
#[cfg(all(feature = "gpu", not(target_os = "android")))]
fn probe_gpu_adapter() -> bool {
    let instance = wgpu::Instance::default();
    pollster_lite::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .is_ok()
}

/// A minimal thread-parking executor, so probing for an adapter does not drag
/// an async runtime into this crate.
///
/// Mirrors the same device in `njoy-outram-park-fork`'s `gpu.rs`; kept private
/// because it exists only to make one `wgpu` call blocking.
#[cfg(all(feature = "gpu", not(target_os = "android")))]
mod pollster_lite {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};

    struct ThreadWaker(std::thread::Thread);

    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    /// Drive `future` to completion on the calling thread, parking between
    /// polls.
    pub fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
        let mut context = Context::from_waker(&waker);
        loop {
            match Pin::new(&mut future).poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::park(),
            }
        }
    }
}

#[cfg(test)]
mod tests;
