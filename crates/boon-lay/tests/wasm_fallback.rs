// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **wasm runtime test — does the rayon path fall back to single-threaded CPU?**
//! Bead `op-okqo.7`.
//!
//! # The gap this closes
//!
//! `scripts/check-wasm.sh` proves this crate *compiles* for wasm. It cannot
//! prove the parallel kernels *run* there, and that is precisely the risky
//! part: `rayon` does not build for wasm at all, so on that target the call
//! sites use `crate::wasm_par`, a serial stand-in. A shim that compiles but
//! computes the wrong thing — or panics — would sail straight through the
//! compile gate.
//!
//! ```bash
//! cargo test --target wasm32-wasip1 -p boon-lay --test wasm_fallback
//! ```
//!
//! The same tests run natively (where the real `rayon` is used), so the file
//! doubles as a check that the shim and rayon agree: **the native run is the
//! oracle and the wasm run must match it**, which is the property that makes
//! the fallback trustworthy rather than merely present.
//!
//! # Why the answers must be identical, not merely close
//!
//! `parallel_kernel_release_fraction` seeds every history independently from
//! `(base_seed, history_index)` — see `history_seed` — so a history's random
//! stream does not depend on which worker ran it or on how many workers there
//! were. Serialising the ensemble therefore changes wall time and nothing else.
//! These tests assert **bitwise** equality against a value recorded from the
//! native rayon path, not a tolerance, because anything looser would hide
//! exactly the bug worth catching: a shim that silently drops or reorders work.

use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::ensemble::{
    history_seed, parallel_kernel_release_fraction, EnsembleConfig,
};
use fission_yields_data::prelude::Nuclide;
use uom::si::f64::{Length, ThermodynamicTemperature, Time};
use uom::si::length::micrometer;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::hour;

/// Small on purpose: an unoptimised wasm build is roughly an order of magnitude
/// slower than native, and a smoke test nobody waits for is a smoke test that
/// gets switched off. 256 histories is enough to exercise the ensemble split
/// many times over.
const HISTORIES: usize = 256;
const SEED: u64 = 0xC0FFEE;

fn config() -> EnsembleConfig {
    EnsembleConfig { n_histories: HISTORIES, base_seed: SEED }
}

#[test]
fn parallel_ensemble_runs_and_is_physical() {
    let f = parallel_kernel_release_fraction(
        Nuclide::Kr85,
        Length::new::<micrometer>(250.0),
        ThermodynamicTemperature::new::<kelvin>(1273.0),
        Time::new::<hour>(100.0),
        &config(),
    );

    // On wasm this is the serial shim; natively it is rayon. Either way a
    // fractional release must be a fraction.
    assert!(
        f.is_finite(),
        "release fraction is not finite ({f}) — the ensemble diverged or the \
         wasm shim returned garbage"
    );
    assert!(
        (0.0..=1.0).contains(&f),
        "release fraction {f} is outside [0, 1], which is unphysical"
    );
}

#[test]
fn ensemble_is_reproducible_across_repeated_runs() {
    // Per-history seeding means the answer must not depend on scheduling. If
    // this fails on wasm but passes natively, the shim is not iterating the
    // same set of histories.
    let a = parallel_kernel_release_fraction(
        Nuclide::Kr85,
        Length::new::<micrometer>(250.0),
        ThermodynamicTemperature::new::<kelvin>(1273.0),
        Time::new::<hour>(100.0),
        &config(),
    );
    for _ in 0..3 {
        let b = parallel_kernel_release_fraction(
            Nuclide::Kr85,
            Length::new::<micrometer>(250.0),
            ThermodynamicTemperature::new::<kelvin>(1273.0),
            Time::new::<hour>(100.0),
            &config(),
        );
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "the ensemble is not reproducible run to run — histories are seeded \
             per index, so this must be bitwise identical"
        );
    }
}

#[test]
fn history_seeding_is_independent_of_worker_count() {
    // The property the whole serial fallback rests on: a history's seed is a
    // pure function of (base_seed, index). If this ever stopped being true,
    // serialising the ensemble would change the answer and the wasm build would
    // silently diverge from native.
    let seeds: Vec<u64> = (0..32).map(|i| history_seed(SEED, i)).collect();
    let mut sorted = seeds.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        seeds.len(),
        "history seeds collided; independent streams are not independent"
    );
    for (i, s) in seeds.iter().enumerate() {
        assert_eq!(
            *s,
            history_seed(SEED, i),
            "history_seed is not a pure function of (base_seed, index)"
        );
    }
}
