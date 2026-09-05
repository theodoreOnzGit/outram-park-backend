// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! **Data-level co-execution** — split one batch between the CPU and the GPU
//! so both run at once. Bead `op-yvj.4.8`, GitHub #18.
//!
//! # What this is, and what it is not
//!
//! This is a **selection policy over the existing backends**, exactly as #18
//! asked: it decides how many lanes go to `CpuMulti` and how many to `Gpu`,
//! then calls the kernels that already exist. It is **not** a fourth
//! implementation of every kernel, and nothing here contains numerical code.
//!
//! Of the three forms #18 identified, this module implements **form 2,
//! data-level co-execution**, because that is the one the issue called "the
//! direct answer to the question and the easiest real speedup". Form 1
//! (task-level overlap of a sequential preconditioner with GPU SpMV) needs the
//! wgpu dispatch to be genuinely async — this crate's
//! [`crate::compute::gpu::GpuContext::dispatch`] blocks on readback — so it is
//! out of scope here and noted in "Not implemented" below. Form 3
//! (transfer/compute overlap) is a scheduling detail of form 2 and is likewise
//! not attempted while dispatch is synchronous.
//!
//! # The two caveats #18 required be settled *before* implementation
//!
//! Both are settled here, in the API rather than in prose.
//!
//! **Mixed precision within one result.** A split batch computes some lanes in
//! `f64` on the CPU and others in `f32` on the GPU, so two adjacent cells can
//! differ in accuracy purely by which side of the split they landed on. That
//! is real and it is not hidden: [`SplitPlan::precision_note`] states it per
//! call, and [`CoExecution::MatchGpuPrecision`] exists for callers who would
//! rather have *uniform* `f32`-grade accuracy across the whole array than a
//! seam in the middle of it. Choosing is the caller's; the default
//! ([`CoExecution::NativePrecision`]) keeps the CPU half at full `f64`,
//! because silently degrading correct arithmetic to match a less accurate
//! path is the worse default.
//!
//! **Reproducibility.** The split is **static and deterministic** by default:
//! [`SplitPlan::for_lanes`] is a pure function of the lane count and the
//! ratio, so the same input gives the same split, and therefore the same
//! answer, on every run. #18 recommended precisely this, and warned that a
//! dynamic work-stealing split reorders reductions and changes the last bits
//! run to run. Work-stealing is therefore **not** offered here; if it is
//! wanted later it belongs behind an explicit opt-in whose docs say bitwise
//! reproducibility is surrendered.
//!
//! # Where the default ratio comes from
//!
//! [`SplitRatio::Measured`] is not a guess. `examples/hybrid_gpu_report.rs`
//! swept 4 096 to 4 194 304 lanes on the reference machine and the GPU lost to
//! `CpuMulti` at **every** size, by 5x to 100x. The honest ratio on that
//! hardware is therefore **zero GPU lanes**, and that is what
//! [`SplitRatio::Measured`] resolves to — a co-execution planner that
//! truthfully reports "do not use the GPU here" is doing its job. A caller on
//! better hardware supplies [`SplitRatio::Fixed`] from their own run of the
//! report.
//!
//! # Not implemented, deliberately
//!
//! - **Task-level concurrency** (#18 form 1) — needs async dispatch.
//! - **Dynamic work stealing** — surrenders reproducibility; see above.
//! - **Transfer/compute overlap** (#18 form 3) — subsumed by the above.

use crate::compute::ComputeBackend;

/// How to choose the CPU/GPU lane split.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SplitRatio {
    /// Use the ratio measured by `examples/hybrid_gpu_report.rs` on the
    /// reference hardware.
    ///
    /// On that machine the measured answer is **0.0** — the GPU never beat
    /// `CpuMulti` — so this resolves to a CPU-only plan. That is a real
    /// measurement, not a placeholder: see the module docs.
    #[default]
    Measured,
    /// A caller-supplied fraction of lanes to send to the GPU, clamped to
    /// `[0.0, 1.0]`. Take it from your own run of the crossover report rather
    /// than guessing.
    Fixed(f64),
}

/// What precision the CPU half of a split batch should run at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoExecution {
    /// CPU lanes stay `f64`, GPU lanes are `f32`. Fastest and most accurate
    /// overall, but the output array is **not uniform in accuracy** — see the
    /// module docs.
    #[default]
    NativePrecision,
    /// Round the CPU half's results to `f32` as well, so every lane in the
    /// output has the same accuracy grade.
    ///
    /// This makes the array *uniform*, not *better*: it throws away good bits
    /// on the CPU lanes to remove the seam. Choose it when downstream code
    /// compares cells against each other and a discontinuity at the split
    /// point would be mistaken for physics.
    MatchGpuPrecision,
}

/// A resolved, deterministic plan for one batch.
///
/// Produced by [`SplitPlan::for_lanes`]. Holds no borrows and no lifetimes,
/// per the workspace rules.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitPlan {
    /// Total lanes in the batch.
    pub lanes: usize,
    /// Lanes `0 .. cpu_lanes` run on the CPU backend.
    pub cpu_lanes: usize,
    /// Lanes `cpu_lanes .. lanes` run on the GPU.
    pub gpu_lanes: usize,
    /// The CPU backend the plan will actually use, already resolved.
    pub cpu_backend: ComputeBackend,
    /// Precision policy for the CPU half.
    pub co_execution: CoExecution,
}

impl SplitPlan {
    /// Build the deterministic plan for `lanes` work items.
    ///
    /// # Arguments
    ///
    /// - `lanes` — total independent work items, dimensionless.
    /// - `ratio` — how much to give the GPU; see [`SplitRatio`].
    /// - `co_execution` — precision policy for the CPU half.
    ///
    /// # Returns
    ///
    /// A plan whose `cpu_lanes + gpu_lanes == lanes` exactly. When the GPU is
    /// unavailable — no `gpu` feature, Android, or no adapter — `gpu_lanes` is
    /// `0` and the whole batch is planned for the CPU; that is the graceful
    /// degradation contract, not an error.
    ///
    /// # Determinism
    ///
    /// A pure function of its arguments and the machine's backend
    /// availability. It performs no timing, samples no load, and consults no
    /// random source, so two runs on one machine always plan identically.
    #[must_use]
    pub fn for_lanes(lanes: usize, ratio: SplitRatio, co_execution: CoExecution) -> Self {
        let cpu_backend = ComputeBackend::CpuMulti.resolve();
        let gpu_fraction = if ComputeBackend::Gpu.is_available() {
            match ratio {
                // Measured on the reference machine: the GPU never won.
                SplitRatio::Measured => 0.0,
                SplitRatio::Fixed(f) => f.clamp(0.0, 1.0),
            }
        } else {
            0.0
        };

        // Round the GPU share down, so any remainder lane goes to the CPU —
        // the more accurate side. Deterministic for a given (lanes, fraction).
        let gpu_lanes = ((lanes as f64) * gpu_fraction).floor() as usize;
        let gpu_lanes = gpu_lanes.min(lanes);
        SplitPlan {
            lanes,
            cpu_lanes: lanes - gpu_lanes,
            gpu_lanes,
            cpu_backend,
            co_execution,
        }
    }

    /// Whether this plan actually uses both sides.
    ///
    /// `false` for a CPU-only or GPU-only plan, which is the common case on
    /// hardware where one side dominates.
    #[must_use]
    pub fn is_co_executing(&self) -> bool {
        self.cpu_lanes > 0 && self.gpu_lanes > 0
    }

    /// A one-line, caller-facing statement of the precision this plan will
    /// produce — the per-call form of the mixed-precision caveat #18 required
    /// be documented rather than discovered.
    ///
    /// Intended to be logged or surfaced by anything recording a V&V run, so a
    /// result set carries the precision story with it.
    #[must_use]
    pub fn precision_note(&self) -> &'static str {
        if !self.is_co_executing() {
            if self.gpu_lanes > 0 {
                "all lanes computed on the GPU in f32"
            } else {
                "all lanes computed on the CPU in f64"
            }
        } else {
            match self.co_execution {
                CoExecution::NativePrecision => {
                    "MIXED PRECISION: leading lanes are f64 (CPU), trailing lanes are f32 (GPU); \
                     accuracy is not uniform across the output array"
                }
                CoExecution::MatchGpuPrecision => {
                    "uniform f32-grade accuracy: CPU lanes deliberately rounded to f32 to match \
                     the GPU half"
                }
            }
        }
    }

    /// Apply the [`CoExecution`] policy to the CPU half of a completed result.
    ///
    /// Call this on the CPU-computed lanes before splicing the two halves
    /// together. Under [`CoExecution::NativePrecision`] it is a no-op; under
    /// [`CoExecution::MatchGpuPrecision`] it rounds each value through `f32`.
    pub fn harmonise_cpu_half(&self, cpu_values: &mut [f64]) {
        if self.co_execution == CoExecution::MatchGpuPrecision {
            for v in cpu_values.iter_mut() {
                *v = (*v as f32) as f64;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_is_exhaustive_and_deterministic() {
        for lanes in [0, 1, 63, 1024, 1_000_003] {
            let a = SplitPlan::for_lanes(lanes, SplitRatio::Fixed(0.3), CoExecution::default());
            let b = SplitPlan::for_lanes(lanes, SplitRatio::Fixed(0.3), CoExecution::default());
            assert_eq!(a, b, "the same inputs must plan identically every time");
            assert_eq!(
                a.cpu_lanes + a.gpu_lanes,
                lanes,
                "every lane must be assigned exactly once"
            );
        }
    }

    #[test]
    fn measured_ratio_is_cpu_only_on_this_hardware() {
        // The reference measurement found the GPU never beat CpuMulti, so the
        // honest planned share is zero. If a future run of the crossover
        // report changes that, this test is the thing that must be revisited
        // deliberately rather than silently.
        let p = SplitPlan::for_lanes(1 << 20, SplitRatio::Measured, CoExecution::default());
        assert_eq!(p.gpu_lanes, 0);
        assert_eq!(p.cpu_lanes, 1 << 20);
        assert!(!p.is_co_executing());
    }

    #[test]
    fn no_gpu_means_every_lane_goes_to_the_cpu() {
        let p = SplitPlan::for_lanes(1000, SplitRatio::Fixed(0.9), CoExecution::default());
        if !ComputeBackend::Gpu.is_available() {
            assert_eq!(p.gpu_lanes, 0, "no adapter must degrade to a CPU-only plan");
            assert_eq!(p.cpu_lanes, 1000);
        } else {
            assert_eq!(p.gpu_lanes, 900);
            assert_eq!(p.cpu_lanes, 100);
        }
    }

    #[test]
    fn ratio_is_clamped_not_wrapped() {
        let hi = SplitPlan::for_lanes(100, SplitRatio::Fixed(5.0), CoExecution::default());
        let lo = SplitPlan::for_lanes(100, SplitRatio::Fixed(-2.0), CoExecution::default());
        assert!(hi.gpu_lanes <= 100);
        assert_eq!(lo.gpu_lanes, 0);
        assert_eq!(lo.cpu_lanes, 100);
    }

    #[test]
    fn remainder_lane_goes_to_the_more_accurate_side() {
        // 10 lanes at 0.35 -> floor(3.5) = 3 GPU, 7 CPU. The half-lane is
        // resolved toward f64, never away from it.
        let p = SplitPlan::for_lanes(10, SplitRatio::Fixed(0.35), CoExecution::default());
        if ComputeBackend::Gpu.is_available() {
            assert_eq!(p.gpu_lanes, 3);
            assert_eq!(p.cpu_lanes, 7);
        }
    }

    #[test]
    fn precision_note_names_the_mixed_case() {
        let cpu_only = SplitPlan::for_lanes(100, SplitRatio::Fixed(0.0), CoExecution::default());
        assert!(cpu_only.precision_note().contains("f64"));

        // Construct the co-executing case directly so the assertion holds on a
        // machine with no adapter too.
        let mixed = SplitPlan {
            lanes: 100,
            cpu_lanes: 50,
            gpu_lanes: 50,
            cpu_backend: ComputeBackend::Serial,
            co_execution: CoExecution::NativePrecision,
        };
        assert!(
            mixed.precision_note().contains("MIXED PRECISION"),
            "the mixed-precision caveat must be stated, not implied"
        );

        let matched = SplitPlan {
            co_execution: CoExecution::MatchGpuPrecision,
            ..mixed
        };
        assert!(matched.precision_note().contains("uniform f32"));
    }

    #[test]
    fn harmonise_rounds_only_when_asked() {
        let original = std::f64::consts::PI;
        let mixed = SplitPlan {
            lanes: 2,
            cpu_lanes: 1,
            gpu_lanes: 1,
            cpu_backend: ComputeBackend::Serial,
            co_execution: CoExecution::NativePrecision,
        };
        let mut v = [original];
        mixed.harmonise_cpu_half(&mut v);
        assert_eq!(v[0], original, "NativePrecision must not touch the values");

        let matched = SplitPlan {
            co_execution: CoExecution::MatchGpuPrecision,
            ..mixed
        };
        let mut v = [original];
        matched.harmonise_cpu_half(&mut v);
        assert_eq!(v[0], (original as f32) as f64);
        assert_ne!(v[0], original, "MatchGpuPrecision must actually round");
    }
}
