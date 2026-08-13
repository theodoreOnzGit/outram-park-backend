//! Compute-backend selector for the Walk-on-Spheres diffusion ensembles.
//!
//! A single [`ComputeType`] value chooses *how* a Lagrangian ensemble advances
//! its independent atom histories — on one CPU thread, across all CPU cores with
//! [`rayon`], or with a `wgpu` GPU compute kernel. The **physics is identical**
//! across backends; only the execution strategy (and, for the GPU path, the
//! floating-point precision) differs. Enum dispatch is used deliberately — no
//! trait objects — so every `match self { … }` site is exhaustively checked at
//! compile time (see the workspace `CLAUDE.md` "No trait objects" rule).
//!
//! This mirrors the `outram-mc-libs` `ComputeType` switcher
//! (`crates/outram-mc-libs/src/physics/compute.rs`, bead op-fla) so the two
//! crates present the same knob to a user. The driver that honours this selector
//! is [`LiveEnsemble::advance_frame`] in
//! [`crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::live`].
//!
//! [`LiveEnsemble::advance_frame`]:
//! crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::live::LiveEnsemble::advance_frame

/// Which compute backend advances a Walk-on-Spheres ensemble.
///
/// The variants map onto the compute modes named `CPUSingleThread` /
/// `CPUMultiThread` / `GPU`; this enum uses idiomatic Rust casing (`Cpu` / `Gpu`)
/// so the crate builds clean under clippy's `upper_case_acronyms` lint.
///
/// | This enum | Meaning |
/// |---|---|
/// | [`CpuSingleThread`](Self::CpuSingleThread) | scalar, single-thread `f64` |
/// | [`CpuMultiThread`](Self::CpuMultiThread)   | rayon-parallel over histories |
/// | [`Gpu`](Self::Gpu)                         | `wgpu` kernel, CPU fallback |
///
/// # Trust model
///
/// [`CpuSingleThread`](Self::CpuSingleThread) is the **trusted, deterministic
/// reference**: raw `f64`, per-history RNG streams derived from a base seed, so a
/// fixed seed gives reproducible output. The other two backends are
/// **acceleration only** and are validated *against* this reference within
/// statistical uncertainty — never trusted above it (the GPU kernel runs in
/// `f32`; see the crate `gpu` module docs).
///
/// # Portability
///
/// The enum and every driver that dispatches on it compile on **all** targets,
/// including Android (`target_os = "android"`), where the GPU module is
/// target-gated out. On Android, [`Gpu`](Self::Gpu) transparently runs the CPU
/// path (there is no adapter to probe), so selecting it is always safe.
/// [`CpuMultiThread`](Self::CpuMultiThread) is a valid Android backend too —
/// `rayon` and [`std::thread::available_parallelism`] both work there; a phone
/// simply resolves to fewer cores.
///
/// `Eq` is deliberately **not** derived: [`ThreadCount::Fraction`] carries an
/// `f64`, which is only `PartialEq`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ComputeType {
    /// Scalar, single-thread advance — the **deterministic trusted reference**.
    ///
    /// One `f64` history at a time; a fixed base seed yields reproducible output
    /// independent of machine. This is the default.
    #[default]
    CpuSingleThread,

    /// Rayon-parallel advance over the atom histories, sized by [`ThreadCount`].
    ///
    /// Histories are embarrassingly parallel, so they run across CPU cores with
    /// [`rayon`] in a **dedicated pool** sized to [`ThreadCount`]
    /// ([`ThreadCount::Auto`] scales with the machine's logical-core count).
    /// Each history's RNG stream is derived deterministically from `(base_seed,
    /// index)`, so the result is reproducible independent of thread count and
    /// does not race. It agrees with [`CpuSingleThread`](Self::CpuSingleThread)
    /// within statistical uncertainty.
    ///
    /// Construct the default form with `CpuMultiThread(ThreadCount::Auto)`.
    CpuMultiThread(ThreadCount),

    /// GPU-accelerated advance via `wgpu`, with graceful CPU fallback.
    ///
    /// Runs the Walk-on-Spheres kernel in `f32` on the GPU. If no GPU adapter is
    /// available — a headless server, CI with no Vulkan/Metal loader, or Android
    /// where the GPU module is compiled out — the driver falls back transparently
    /// to the CPU path. It **never errors on a missing GPU.** GPU `f32` results
    /// are acceleration only and are held to a tolerance against the CPU
    /// reference.
    Gpu,
}

/// How many worker threads the [`ComputeType::CpuMultiThread`] backend uses,
/// sized to the CPU's strength.
///
/// The driver resolves this to a concrete positive thread count with
/// [`ThreadCount::resolve`] and builds a dedicated [`rayon::ThreadPool`] of that
/// size. The default is [`Auto`](Self::Auto), which reads the machine's logical
/// core count via [`std::thread::available_parallelism`] — a desktop naturally
/// gets many threads, a phone gets few, with no special-casing. All variants
/// resolve to **at least 1** thread.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ThreadCount {
    /// Use every logical core: [`std::thread::available_parallelism`]. Falls
    /// back to 1 if the query fails. The default.
    #[default]
    Auto,
    /// An explicit worker-thread count. Clamped up to a minimum of 1.
    Fixed(usize),
    /// A fraction of the available logical cores, e.g. `0.5` = half. The product
    /// `fraction * cores` is rounded to the nearest integer and clamped to at
    /// least 1.
    Fraction(f64),
}

impl ThreadCount {
    /// Resolve to a concrete worker-thread count (always `>= 1`).
    ///
    /// - [`Auto`](Self::Auto) → the logical core count from
    ///   [`std::thread::available_parallelism`], or `1` if that query fails.
    /// - [`Fixed(n)`](Self::Fixed) → `n.max(1)`.
    /// - [`Fraction(f)`](Self::Fraction) → `round(f * cores)`, clamped to `>= 1`.
    ///   A non-finite or non-positive fraction resolves to `1`.
    ///
    /// Pure and Android-safe (`available_parallelism` is `std` and works on
    /// Android — a phone just reports fewer cores).
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

impl ComputeType {
    /// A short human-readable label for the selected backend, for UI display.
    pub fn label(self) -> &'static str {
        match self {
            ComputeType::CpuSingleThread => "CPU (single-thread, deterministic reference)",
            ComputeType::CpuMultiThread(_) => "CPU (multi-thread, rayon over all cores)",
            ComputeType::Gpu => "GPU (wgpu; falls back to CPU if no adapter)",
        }
    }

    /// The next backend in the cycle single → multi → GPU → single, for a
    /// tap-to-rotate UI control (mirrors the `outram-mc-tui` idiom).
    pub fn next(self) -> ComputeType {
        match self {
            ComputeType::CpuSingleThread => ComputeType::CpuMultiThread(ThreadCount::Auto),
            ComputeType::CpuMultiThread(_) => ComputeType::Gpu,
            ComputeType::Gpu => ComputeType::CpuSingleThread,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_is_always_at_least_one() {
        assert!(ThreadCount::Auto.resolve() >= 1);
        assert_eq!(ThreadCount::Fixed(0).resolve(), 1);
        assert_eq!(ThreadCount::Fixed(4).resolve(), 4);
        assert!(ThreadCount::Fraction(0.5).resolve() >= 1);
        // Degenerate fractions clamp to 1, never panic or return 0.
        assert_eq!(ThreadCount::Fraction(0.0).resolve(), 1);
        assert_eq!(ThreadCount::Fraction(-2.0).resolve(), 1);
        assert_eq!(ThreadCount::Fraction(f64::NAN).resolve(), 1);
    }

    #[test]
    fn fraction_one_matches_auto() {
        assert_eq!(
            ThreadCount::Fraction(1.0).resolve(),
            ThreadCount::Auto.resolve()
        );
    }

    #[test]
    fn default_is_cpu_single_thread() {
        assert_eq!(ComputeType::default(), ComputeType::CpuSingleThread);
    }

    #[test]
    fn next_cycles_through_all_three() {
        let a = ComputeType::default();
        let b = a.next();
        let c = b.next();
        let d = c.next();
        assert_eq!(a, ComputeType::CpuSingleThread);
        assert!(matches!(b, ComputeType::CpuMultiThread(_)));
        assert_eq!(c, ComputeType::Gpu);
        assert_eq!(d, ComputeType::CpuSingleThread); // wraps around
    }
}
