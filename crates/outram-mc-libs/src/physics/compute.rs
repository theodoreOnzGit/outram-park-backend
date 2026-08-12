//! Compute-backend selector for the Monte Carlo transport drivers.
//!
//! A single [`ComputeType`] value chooses *how* a k-eigenvalue power iteration
//! executes its per-generation history transport — on one CPU thread, across all
//! CPU cores with [`rayon`], or with GPU-accelerated cross-section lookups. The
//! **physics is identical** across backends; only the execution strategy (and,
//! for the GPU path, the floating-point precision of the cross-section lookup)
//! differs. Enum dispatch is used deliberately — no trait objects — so every
//! `match self { … }` site is exhaustively checked at compile time (see the
//! workspace `CLAUDE.md` "No trait objects" rule).
//!
//! The driver that honours this selector is
//! [`run_keff`](crate::physics::keff::run_keff): its [`KeffSettings::compute`]
//! field is matched on to pick the per-mode entry point
//! ([`run_keff_cpu_single`], [`run_keff_cpu_multi`], [`run_keff_gpu`] in
//! [`crate::physics::keff`]).
//!
//! [`KeffSettings::compute`]: crate::physics::keff::KeffSettings::compute
//! [`run_keff_cpu_single`]: crate::physics::keff::run_keff_cpu_single
//! [`run_keff_cpu_multi`]: crate::physics::keff::run_keff_cpu_multi
//! [`run_keff_gpu`]: crate::physics::keff::run_keff_gpu

/// Which compute backend a Monte Carlo transport driver uses to execute its
/// per-generation neutron histories.
///
/// The variants map onto the compute modes the project's maintainer named as
/// `CPUSingleThread` / `CPUMultiThread` / `GPU`; this enum uses idiomatic Rust
/// casing (`Cpu` / `Gpu`) so the crate builds clean under clippy's
/// `upper_case_acronyms` lint. The semantics are exactly those three modes:
///
/// | This enum | Maintainer's name | Meaning |
/// |---|---|---|
/// | [`CpuSingleThread`](Self::CpuSingleThread) | `CPUSingleThread` | scalar, single-thread |
/// | [`CpuMultiThread`](Self::CpuMultiThread)  | `CPUMultiThread`  | rayon-parallel over histories |
/// | [`Gpu`](Self::Gpu)                         | `GPU`             | GPU-accelerated XS lookup, CPU fallback |
///
/// # Trust model
///
/// [`CpuSingleThread`](Self::CpuSingleThread) is the **trusted, deterministic
/// reference**: raw `f64` throughout, a single RNG stream threaded through the
/// generation, so a fixed seed gives a bit-reproducible k-eigenvalue. The other
/// two backends are **acceleration only** and are validated *against* this
/// reference within combined statistical uncertainty — never trusted above it
/// (see this crate's `gpu` module docs and `CLAUDE.md`).
///
/// # Portability
///
/// The enum and every driver that dispatches on it compile on **all** targets,
/// including Android (`target_os = "android"`), where the GPU module is
/// target-gated out. On Android, [`Gpu`](Self::Gpu) transparently runs the CPU
/// path (there is no adapter to probe), so selecting it is always safe.
/// [`CpuMultiThread`](Self::CpuMultiThread) is a valid Android backend too —
/// `rayon` and [`std::thread::available_parallelism`] both work there; a phone
/// simply resolves to fewer cores, hence fewer threads.
///
/// `Eq` is deliberately **not** derived: [`ThreadCount::Fraction`] carries an
/// `f64`, which is only `PartialEq`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ComputeType {
    /// Scalar, single-thread transport (maintainer's `CPUSingleThread`).
    ///
    /// The current, original behaviour and the **deterministic trusted
    /// reference**: one `f64` RNG stream threaded sequentially through the
    /// generation, so a fixed [`KeffSettings::seed`](crate::physics::keff::KeffSettings::seed)
    /// yields a bit-reproducible k-eigenvalue independent of machine. This is
    /// the default.
    #[default]
    CpuSingleThread,

    /// Rayon-parallel transport over the per-generation history bank
    /// (maintainer's `CPUMultiThread`), sized by the carried [`ThreadCount`].
    ///
    /// Histories within a generation are embarrassingly parallel, so they are
    /// transported across CPU cores with [`rayon`] in a **dedicated pool** sized
    /// to [`ThreadCount`] — [`ThreadCount::Auto`] (the default) scales with the
    /// machine's logical-core count, so a big desktop CPU gets many threads and
    /// a phone gets few with no special-casing. Each history is given an RNG
    /// stream derived deterministically from `(seed, generation, history index)`
    /// via the LCG jump-ahead, so the result is **reproducible independent of
    /// thread count** and does not race. It will **not** bit-match
    /// [`CpuSingleThread`](Self::CpuSingleThread) — the per-history stream
    /// structure differs from the single sequential stream — but it agrees
    /// within combined statistical uncertainty.
    ///
    /// Construct the default form with `CpuMultiThread(ThreadCount::Auto)`.
    CpuMultiThread(ThreadCount),

    /// GPU-accelerated transport (maintainer's `GPU`), with graceful CPU
    /// fallback.
    ///
    /// Uses the headless `wgpu` cross-section lookup
    /// ([`crate::gpu::union_grid::UnionTotalXs`]) to serve the macroscopic total
    /// Sigma_t during the sweep on the GPU in `f32`. If no GPU adapter is
    /// available — a headless server, CI with no Vulkan/Metal loader, or Android
    /// where the GPU module is compiled out — the driver emits a
    /// `log::debug!` line and transparently runs the CPU path instead. It
    /// **never errors on a missing GPU.** GPU `f32` results are acceleration
    /// only and are held to a tolerance against the CPU reference.
    Gpu,
}

/// How many worker threads the [`ComputeType::CpuMultiThread`] backend uses,
/// sized to the CPU's strength.
///
/// The transport driver resolves this to a concrete positive thread count with
/// [`ThreadCount::resolve`] and builds a dedicated [`rayon::ThreadPool`] of that
/// size. The default is [`Auto`](Self::Auto), which reads the machine's logical
/// core count via [`std::thread::available_parallelism`] — a gaming desktop
/// naturally gets many threads, an Android phone gets few, with no
/// special-casing. All variants resolve to **at least 1** thread.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ThreadCount {
    /// Use every logical core: [`std::thread::available_parallelism`]. Scales
    /// with the CPU's strength; falls back to 1 if the query fails. The default.
    #[default]
    Auto,
    /// An explicit worker-thread count. Clamped up to a minimum of 1.
    Fixed(usize),
    /// A fraction of the available logical cores, e.g. `0.5` = half. The product
    /// `fraction * cores` is rounded to the nearest integer and clamped to at
    /// least 1 (so any positive fraction always yields a runnable pool).
    Fraction(f64),
}

impl ThreadCount {
    /// Resolve to a concrete worker-thread count (always `>= 1`).
    ///
    /// - [`Auto`](Self::Auto) → the logical core count from
    ///   [`std::thread::available_parallelism`], or `1` if that query fails.
    /// - [`Fixed(n)`](Self::Fixed) → `n.max(1)`.
    /// - [`Fraction(f)`](Self::Fraction) → `round(f * cores)`, clamped to `>= 1`,
    ///   where `cores` is the [`Auto`](Self::Auto) count. A non-finite or
    ///   non-positive fraction resolves to `1`.
    ///
    /// This is pure and Android-safe (`available_parallelism` is `std` and works
    /// on Android — a phone just reports fewer cores).
    pub fn resolve(self) -> usize {
        fn logical_cores() -> usize {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        }
        match self {
            ThreadCount::Auto => logical_cores(),
            ThreadCount::Fixed(n) => n.max(1),
            ThreadCount::Fraction(f) => {
                if !f.is_finite() || f <= 0.0 {
                    return 1;
                }
                ((f * logical_cores() as f64).round() as usize).max(1)
            }
        }
    }
}
