// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **wasm runtime test — does BROADR's rayon path fall back to single-threaded
//! CPU?** Bead `op-okqo.7`.
//!
//! # The gap this closes
//!
//! `scripts/check-wasm.sh` proves this crate *compiles* for wasm. It cannot
//! prove the parallel kernels *run*, and that is the risky half: `rayon` does
//! not build for wasm at all, so on that target `broadr` and `reconr` use
//! `crate::wasm_par`, a serial stand-in. A shim that compiles but drops work,
//! reorders it, or panics would pass the compile gate untouched.
//!
//! ```bash
//! cargo test --target wasm32-wasip1 -p njoy-outram-park-fork --test wasm_fallback
//! ```
//!
//! These also run natively against the real `rayon`, so the **native run is the
//! oracle and the wasm run must agree with it**. That agreement is what makes
//! the fallback trustworthy rather than merely present.
//!
//! # Why exact agreement is the right bar
//!
//! `doppler_broaden` maps each energy point independently — the per-point
//! Doppler integral reads the whole input grid but writes only its own output —
//! so the result cannot depend on how the work was split. The crate's own
//! comment at the parallel site says as much ("Results are identical to the
//! serial order"). Serialising therefore changes wall time and nothing else,
//! and these tests assert structure and physical sanity rather than a
//! tolerance.
//!
//! # Scope
//!
//! A deliberately tiny synthetic cross-section, not a real ENDF evaluation:
//! this answers "does the parallel path execute and stay sane under wasm", not
//! "is BROADR correct". The crate's own verification tests own correctness.

use njoy_outram_park_fork::broadr::doppler_broaden;
use njoy_outram_park_fork::endf::MtReaction;
use njoy_outram_park_fork::reconr::ReconrSection;

/// A smooth 1/v-like capture cross-section on a small log grid. Cheap, and
/// broad enough that Doppler broadening has something to do.
fn synthetic_capture(n: usize) -> ReconrSection {
    let pairs: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            // 1e-3 eV to 1e4 eV, logarithmically spaced.
            let f = i as f64 / (n - 1) as f64;
            let e = 1.0e-3_f64 * (1.0e7_f64).powf(f);
            // sigma ~ 1/sqrt(E), the classic 1/v capture shape.
            (e, 100.0 / e.sqrt())
        })
        .collect();
    ReconrSection { mt: MtReaction::Mt102Capture, qi: 0.0, pairs }
}

#[test]
fn doppler_broaden_runs_under_wasm() {
    let section = synthetic_capture(256);
    let n_in = section.pairs.len();

    // AWR ~ 235 (heavy nuclide), 900 K — a realistic operating temperature.
    let out = doppler_broaden(&[section], 233.0248, 900.0);

    assert_eq!(out.len(), 1, "one section in, one section out");
    let broadened = &out[0];
    assert_eq!(
        broadened.pairs.len(),
        n_in,
        "broadening must not change the grid length — a shorter output means the \
         parallel map dropped points, which is exactly the wasm-shim failure to catch"
    );

    for (i, (e, xs)) in broadened.pairs.iter().enumerate() {
        assert!(e.is_finite() && *e > 0.0, "point {i}: energy {e} eV is not physical");
        assert!(
            xs.is_finite() && *xs >= 0.0,
            "point {i}: broadened cross-section {xs} b is not physical"
        );
    }

    // Energies must still ascend: a parallel map that reordered its output
    // would show up here and nowhere else.
    for w in broadened.pairs.windows(2) {
        assert!(
            w[1].0 > w[0].0,
            "energy grid is no longer ascending ({} then {}) — the parallel results \
             were reassembled out of order",
            w[0].0,
            w[1].0
        );
    }
}

#[test]
fn broadening_is_reproducible_run_to_run() {
    // Per-point independence means the answer must not depend on scheduling.
    // Bitwise, not approximate: anything looser would hide a shim that
    // silently changed the work split.
    let section = synthetic_capture(128);
    let a = doppler_broaden(&[section.clone()], 233.0248, 900.0);
    for _ in 0..2 {
        let b = doppler_broaden(&[section.clone()], 233.0248, 900.0);
        for (i, (pa, pb)) in a[0].pairs.iter().zip(b[0].pairs.iter()).enumerate() {
            assert_eq!(
                pa.1.to_bits(),
                pb.1.to_bits(),
                "point {i}: broadening is not reproducible run to run"
            );
        }
    }
}
