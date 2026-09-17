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

//! Compute-backend selector for the DEM timestep and the bulk analysis kernels.
//!
//! A single [`ComputeType`] value chooses *how* work executes — on one CPU
//! thread, across all CPU cores with [`rayon`], or with a GPU kernel where one
//! exists. The **physics is identical** across backends; only the execution
//! strategy differs. Enum dispatch is used deliberately — no trait objects —
//! so every `match self { … }` site is exhaustively checked at compile time
//! (workspace `CLAUDE.md`, "No trait objects").
//!
//! This mirrors [`outram_mc_libs::physics::compute::ComputeType`] deliberately,
//! so the two pillars of the pebble-bed workflow present the same selector to a
//! caller. **One semantic differs, and it matters** — see the trust model
//! below.
//!
//! # Trust model — stricter here than in the Monte Carlo pillar
//!
//! In `outram-mc-libs` the multi-thread backend is documented as agreeing with
//! the single-thread reference only *within statistical uncertainty*, because
//! parallel transport restructures the RNG streams.
//!
//! **This crate holds its parallel backend to bit-identity instead**, and
//! tests it. That is not gold-plating: the crate's entire verification claim
//! is that it reproduces upstream LIGGGHTS *bit-for-bit* on the deterministic
//! cases and to 61 µm median per-pebble on the HTR-10 bed. A backend that
//! perturbed the last bits would force every one of those comparisons to be
//! re-qualified per backend, and would quietly convert an exact claim into a
//! tolerance. So [`CpuMultiThread`](ComputeType::CpuMultiThread) is required to
//! reproduce [`CpuSingleThread`](ComputeType::CpuSingleThread) exactly, for
//! any thread count.
//!
//! Achieving that is a design constraint on the force loop, not an accident:
//! floating-point addition is not associative, so the parallel path computes
//! per-contact forces concurrently but **accumulates them in the serial pair
//! order**. See [`crate::granular_system::GranularSystem::compute_forces`].
//!
//! # Portability
//!
//! The enum and every driver that dispatches on it compile on **all** targets.
//! On `wasm32` (no OS threads) the rayon call sites use [`crate::wasm_par`] and
//! run serially — which, given the bit-identity property above, is *numerically
//! exact*, not a degraded fallback. On Android the GPU module is target-gated
//! out and [`Gpu`](ComputeType::Gpu) transparently runs the CPU path, so
//! selecting it is always safe.
//!
//! `Eq` is deliberately **not** derived: [`ThreadCount::Fraction`] carries an
//! `f64`, which is only `PartialEq`.

/// Which compute backend a DEM driver or bulk analysis kernel uses.
///
/// | This enum | Meaning |
/// |---|---|
/// | [`CpuSingleThread`](Self::CpuSingleThread) | scalar, single thread — the trusted reference |
/// | [`CpuMultiThread`](Self::CpuMultiThread) | rayon-parallel, **bit-identical** to the reference |
/// | [`Gpu`](Self::Gpu) | GPU kernel where one exists, CPU fallback otherwise |
///
/// # Which kernels honour it
///
/// | Kernel | `CpuSingleThread` | `CpuMultiThread` | `Gpu` |
/// |---|---|---|---|
/// | [`GranularSystem::step`](crate::granular_system::GranularSystem::step) | yes | yes | falls back to CPU |
/// | [`rdf`](crate::rdf) pair-separation histogram | yes | yes | **yes** |
///
/// **The timestep has no GPU path, deliberately.** DEM's expensive inner loop
/// carries a persistent per-contact tangential shear history
/// ([`ShearHistory`](crate::granular::ShearHistory)) with contacts born and
/// dying every step — a stateful gather/scatter structure that is the *worst*
/// shape for a shader, while the Hertz force arithmetic that would port well is
/// not where the time goes. At HTR-10 scale (27 554 pebbles, ~165 000 live
/// contacts) the per-step working set is also far too small to amortise a
/// host↔device round trip unless the entire integrator became GPU-resident.
/// Selecting [`Gpu`](Self::Gpu) for a timestep is therefore not an error — it
/// runs the CPU path.
///
/// The radial distribution function is the opposite shape, which is why it has
/// a real GPU kernel: an all-pairs distance histogram over 27 554 positions is
/// 3.8e8 stateless, independent pair evaluations.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ComputeType {
    /// Scalar, single-thread execution — the **deterministic trusted
    /// reference**, and the default.
    ///
    /// Every committed cross-code comparison in
    /// `crates/outram-park-fork-liggghts/docs/` was produced on this backend.
    #[default]
    CpuSingleThread,

    /// Rayon-parallel execution, sized by the carried [`ThreadCount`].
    ///
    /// **Bit-identical to [`CpuSingleThread`](Self::CpuSingleThread)** for any
    /// thread count — see the module-level trust model for why that is required
    /// rather than merely nice, and
    /// [`GranularSystem::compute_forces`](crate::granular_system::GranularSystem::compute_forces)
    /// for how the accumulation order is preserved.
    ///
    /// Work runs in a **dedicated pool** sized to [`ThreadCount`], never the
    /// implicit global rayon pool, so a caller that is itself inside a rayon
    /// scope cannot deadlock or oversubscribe.
    ///
    /// Construct the default form with `CpuMultiThread(ThreadCount::Auto)`.
    CpuMultiThread(ThreadCount),

    /// GPU-accelerated execution where a kernel exists, with graceful CPU
    /// fallback.
    ///
    /// Today exactly one kernel is GPU-backed: the [`rdf`](crate::rdf) pair
    /// separation histogram. Everything else runs the CPU path. If no GPU
    /// adapter is available — a headless server, CI with no Vulkan loader, or
    /// Android where the GPU module is compiled out — the driver falls back to
    /// the CPU path. It **never errors on a missing GPU.**
    ///
    /// GPU results are acceleration only and are held to a tolerance against
    /// the CPU reference, never trusted above it.
    Gpu,
}

/// How many worker threads the [`ComputeType::CpuMultiThread`] backend uses,
/// sized to the CPU's strength.
///
/// Resolved to a concrete positive count with [`ThreadCount::resolve`], which
/// then sizes a dedicated [`rayon::ThreadPool`]. The default is
/// [`Auto`](Self::Auto), which reads the machine's logical core count via
/// [`std::thread::available_parallelism`] — a desktop naturally gets many
/// threads, an Android phone gets few, with no special-casing. All variants
/// resolve to **at least 1**.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ThreadCount {
    /// Use every logical core: [`std::thread::available_parallelism`]. Falls
    /// back to 1 if the query fails. The default.
    #[default]
    Auto,
    /// An explicit worker-thread count. Clamped up to a minimum of 1.
    Fixed(usize),
    /// A fraction of the available logical cores, e.g. `0.5` = half. The
    /// product `fraction * cores` is rounded to nearest and clamped to at least
    /// 1, so any positive fraction yields a runnable pool.
    Fraction(f64),
}

impl ThreadCount {
    /// Resolve to a concrete worker-thread count (always `>= 1`).
    ///
    /// - [`Auto`](Self::Auto) → the logical core count, or `1` if the query
    ///   fails.
    /// - [`Fixed(n)`](Self::Fixed) → `n.max(1)`.
    /// - [`Fraction(f)`](Self::Fraction) → `round(f * cores)`, clamped `>= 1`.
    ///   A non-finite or non-positive fraction resolves to `1`.
    ///
    /// Pure and Android-safe (`available_parallelism` is `std` and works on
    /// Android — a phone simply reports fewer cores).
    #[must_use]
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
    /// The worker-thread count this backend will actually use (always `>= 1`).
    ///
    /// `1` for [`CpuSingleThread`](Self::CpuSingleThread) and for
    /// [`Gpu`](Self::Gpu) (whose CPU fallback path is scalar); the resolved
    /// [`ThreadCount`] for [`CpuMultiThread`](Self::CpuMultiThread).
    #[must_use]
    pub fn threads(self) -> usize {
        match self {
            ComputeType::CpuSingleThread | ComputeType::Gpu => 1,
            ComputeType::CpuMultiThread(tc) => tc.resolve(),
        }
    }

    /// Whether this backend runs the DEM timestep on more than one thread.
    ///
    /// [`Gpu`](Self::Gpu) answers `false`: the timestep has no GPU kernel and
    /// falls back to the scalar CPU path (see the type docs for why).
    #[must_use]
    pub fn is_parallel_step(self) -> bool {
        matches!(self, ComputeType::CpuMultiThread(tc) if tc.resolve() > 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_count_always_resolves_to_at_least_one() {
        assert!(ThreadCount::Auto.resolve() >= 1);
        assert_eq!(ThreadCount::Fixed(0).resolve(), 1);
        assert_eq!(ThreadCount::Fixed(7).resolve(), 7);
        assert_eq!(ThreadCount::Fraction(0.0).resolve(), 1);
        assert_eq!(ThreadCount::Fraction(-2.0).resolve(), 1);
        assert_eq!(ThreadCount::Fraction(f64::NAN).resolve(), 1);
        assert!(ThreadCount::Fraction(1.0).resolve() >= 1);
    }

    #[test]
    fn fraction_is_a_fraction_of_auto() {
        let cores = ThreadCount::Auto.resolve();
        let half = ThreadCount::Fraction(0.5).resolve();
        assert_eq!(half, ((0.5 * cores as f64).round() as usize).max(1));
    }

    #[test]
    fn default_backend_is_the_trusted_single_thread_reference() {
        assert_eq!(ComputeType::default(), ComputeType::CpuSingleThread);
        assert_eq!(ComputeType::default().threads(), 1);
        assert!(!ComputeType::default().is_parallel_step());
    }

    #[test]
    fn gpu_reports_a_scalar_cpu_step_because_the_timestep_has_no_gpu_kernel() {
        assert_eq!(ComputeType::Gpu.threads(), 1);
        assert!(!ComputeType::Gpu.is_parallel_step());
    }

    #[test]
    fn a_one_thread_pool_is_not_a_parallel_step() {
        assert!(!ComputeType::CpuMultiThread(ThreadCount::Fixed(1)).is_parallel_step());
        assert!(ComputeType::CpuMultiThread(ThreadCount::Fixed(4)).is_parallel_step());
    }
}
