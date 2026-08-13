// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the compute-backend dispatch layer.
//!
//! These are verification tests, not validation: they check that the dispatch
//! layer obeys its own stated contract. No physics is involved.

use super::*;

/// The serial backend is available unconditionally.
///
/// **Methodology.** Assert [`ComputeBackend::Serial`] reports available and is
/// the `Default`, on whatever target and feature set this build used. Pass
/// criterion: both hold.
///
/// **Results (2026-08-12).** Both hold. This is the load-bearing guarantee
/// behind [`ComputeBackend::resolve`] never failing — if it ever broke, every
/// fallback path in the crate would have nowhere to land.
#[test]
fn serial_is_always_available_and_default() {
    assert!(ComputeBackend::Serial.is_available());
    assert_eq!(ComputeBackend::default(), ComputeBackend::Serial);
}

/// Resolution never yields an unavailable backend, from any starting point.
///
/// **Methodology.** For each of the three variants, call
/// [`ComputeBackend::resolve`] and assert the result reports available. This
/// is the "graceful absence of a GPU" contract: on this machine the GPU is
/// almost certainly absent, so `Gpu` must degrade rather than fail. Pass
/// criterion: every resolved backend is available.
///
/// **Results (2026-08-12).** All three resolve to an available backend. In
/// this container the probe finds no adapter, so `Gpu` degrades — which is
/// exactly the path a headless CI machine takes, and it is exercised here
/// rather than assumed.
#[test]
fn resolve_never_returns_an_unavailable_backend() {
    for requested in [
        ComputeBackend::Serial,
        ComputeBackend::CpuMulti,
        ComputeBackend::Gpu,
    ] {
        let got = requested.resolve();
        assert!(
            got.is_available(),
            "{} resolved to {}, which is not available",
            requested.label(),
            got.label()
        );
    }
}

/// A requested backend never resolves *upward* to a more capable one.
///
/// **Methodology.** `Serial` must resolve to `Serial` — asking for the oracle
/// must never silently hand back an accelerated path, because verification
/// runs depend on getting the reference implementation they asked for. Pass
/// criterion: `Serial.resolve() == Serial`.
///
/// **Results (2026-08-12).** Holds. This matters more than it looks: a V&V
/// gate that asked for `Serial` and silently got `CpuMulti` would be checking
/// the accelerated path against itself.
#[test]
fn resolve_never_upgrades_serial() {
    assert_eq!(ComputeBackend::Serial.resolve(), ComputeBackend::Serial);
}

/// Small problems always run serially, whatever is compiled in.
///
/// **Methodology.** Call [`select_backend`] with work-item counts below
/// [`CPU_MULTI_MIN_WORK_ITEMS`] and assert it returns
/// [`ComputeBackend::Serial`]. Inputs: 0, 1, 16, and one item below the
/// threshold. Pass criterion: all four give `Serial`.
///
/// **Results (2026-08-12).** All four give `Serial`. This is the guard against
/// the classic mistake of paying thread-pool or PCIe overhead to add two
/// 50-element vectors.
#[test]
fn select_backend_keeps_small_problems_serial() {
    for n in [0, 1, 16, CPU_MULTI_MIN_WORK_ITEMS - 1] {
        assert_eq!(
            select_backend(n),
            ComputeBackend::Serial,
            "{n} work items should stay serial"
        );
    }
}

/// Whatever the policy picks is runnable, at every scale.
///
/// **Methodology.** Sweep work-item counts across both thresholds and assert
/// each selection reports available. Inputs: 0 through 1e6, spanning
/// [`CPU_MULTI_MIN_WORK_ITEMS`] and [`GPU_MIN_WORK_ITEMS`]. Pass criterion:
/// every selection is available.
///
/// **Results (2026-08-12).** Every selection is available across the sweep.
#[test]
fn select_backend_only_returns_available_backends() {
    for n in [
        0,
        1,
        CPU_MULTI_MIN_WORK_ITEMS - 1,
        CPU_MULTI_MIN_WORK_ITEMS,
        GPU_MIN_WORK_ITEMS - 1,
        GPU_MIN_WORK_ITEMS,
        1_000_000,
    ] {
        assert!(
            select_backend(n).is_available(),
            "{n} work items selected an unavailable backend"
        );
    }
}

/// The two thresholds are ordered, and GPU needs strictly more work than
/// multi-CPU to be worth it.
///
/// **Methodology.** Assert `CPU_MULTI_MIN_WORK_ITEMS < GPU_MIN_WORK_ITEMS`.
/// Pass criterion: strict inequality.
///
/// **Results (2026-08-12).** `4096 < 65536` holds. If this ever inverted,
/// [`select_backend`] would offer the GPU for problems too small even for
/// threads, which is the wrong way round: a GPU dispatch additionally pays a
/// host-device round trip.
#[test]
fn gpu_threshold_is_above_the_cpu_multi_threshold() {
    assert!(
        CPU_MULTI_MIN_WORK_ITEMS < GPU_MIN_WORK_ITEMS,
        "CPU_MULTI_MIN_WORK_ITEMS ({CPU_MULTI_MIN_WORK_ITEMS}) must be below \
         GPU_MIN_WORK_ITEMS ({GPU_MIN_WORK_ITEMS})"
    );
}

/// Thread counts resolve to at least one worker, including from degenerate
/// requests.
///
/// **Methodology.** Resolve `Auto`, `Fixed(0)`, `Fixed(1)`, `Fixed(7)`, and
/// `Fraction` values including 0.0, a negative, NaN and infinity. Pass
/// criterion: every result is `>= 1`, and the exact values are as documented
/// for `Fixed`.
///
/// **Results (2026-08-12).** All resolve to `>= 1`; `Fixed(0)` clamps to 1 and
/// `Fixed(7)` gives 7. The degenerate fractions clamp to 1 rather than
/// panicking or producing 0 — a zero would be handed straight to a thread-pool
/// builder, which is exactly the sort of input that turns into a confusing
/// panic deep inside rayon.
#[test]
fn thread_count_always_resolves_to_at_least_one() {
    assert_eq!(ThreadCount::Fixed(0).resolve(), 1);
    assert_eq!(ThreadCount::Fixed(1).resolve(), 1);
    assert_eq!(ThreadCount::Fixed(7).resolve(), 7);
    assert!(ThreadCount::Auto.resolve() >= 1);
    assert_eq!(ThreadCount::default(), ThreadCount::Auto);

    for f in [0.0, -1.0, f64::NAN, f64::INFINITY, 0.5, 1.0, 2.0] {
        assert!(
            ThreadCount::Fraction(f).resolve() >= 1,
            "Fraction({f}) resolved below 1"
        );
    }
}

/// The GPU probe is cached, so it cannot cost anything after the first call.
///
/// **Methodology.** Call [`gpu_adapter_present`] twice and assert the answers
/// agree. Pass criterion: identical results, and no panic on a machine with no
/// adapter.
///
/// **Results (2026-08-12).** Both calls agree. In this container the result is
/// `false` (no GPU adapter), which exercises the fallback path rather than the
/// GPU path — the GPU path itself remains unexecuted and unverified here, and
/// bead `op-b4a.4.8` already tracks validating wgpu dispatch on a real GPU
/// host for the workspace generally.
#[test]
fn gpu_probe_is_cached_and_never_panics() {
    let first = gpu_adapter_present();
    let second = gpu_adapter_present();
    assert_eq!(first, second);
}

/// Backend labels are distinct, so a benchmark table cannot conflate two rows.
///
/// **Methodology.** Collect the three labels and assert all differ and none is
/// empty. Pass criterion: three distinct non-empty strings.
///
/// **Results (2026-08-12).** `serial`, `cpu-multi`, `gpu` — distinct.
#[test]
fn backend_labels_are_distinct() {
    let labels = [
        ComputeBackend::Serial.label(),
        ComputeBackend::CpuMulti.label(),
        ComputeBackend::Gpu.label(),
    ];
    for (i, a) in labels.iter().enumerate() {
        assert!(!a.is_empty());
        for b in &labels[i + 1..] {
            assert_ne!(a, b, "duplicate backend label {a}");
        }
    }
}
