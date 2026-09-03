// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **wasm runtime smoke test — the Edwards–O'Brien blowdown physics path.**
//! Bead `op-okqo.7`.
//!
//! # What this is for
//!
//! `scripts/check-wasm.sh` proves the crate *compiles* for wasm. It cannot
//! prove anything *runs*, because `std::thread::spawn`, `Instant::now` and
//! `std::fs` all compile for wasm and fail only at run time. This file closes
//! that gap for the single heaviest physics path in the crate.
//!
//! Run it with:
//!
//! ```bash
//! cargo test --target wasm32-wasip1 -p tampines-steam-tables --test wasm_smoke
//! ```
//!
//! (`.cargo/config.toml` wires Node's built-in WASI as the runner, so no extra
//! tooling is needed. It also passes natively, and is run natively by the
//! ordinary `cargo test`, so it cannot rot unnoticed.)
//!
//! # This is a SMOKE test, not the benchmark
//!
//! The real V&V case is `tests/edwards_blowdown.rs`: 24 cells, 600 ms of
//! transient at a 30 us step — about 20 000 steps — compared against the
//! digitised Edwards & O'Brien gauge traces, and writing CSV output.
//!
//! **None of that is repeated here, and this file makes no accuracy claim.**
//! It runs the same solver on the same geometry for a handful of steps and
//! asserts only that the run is *physically sane and numerically alive*:
//! pressures stay finite, stay positive, and fall (a blowdown must
//! depressurise). It answers "does the PIMPLE path execute under wasm", not
//! "is the answer right". For the latter, read the benchmark test.
//!
//! The step count is kept tiny deliberately: a wasm build is unoptimised and
//! roughly an order of magnitude slower than native, and a smoke test that
//! takes minutes is a smoke test people switch off.
//!
//! # Why WASI and not a browser
//!
//! See `scripts/wasi-run.mjs`. In short: `wasm32-wasip1` shares
//! `target_arch = "wasm32"` so every cfg gate in the workspace applies
//! identically, but unlike `wasm32-unknown-unknown` it can host a test harness.
//! WASI *does* have a clock and a filesystem, so this does not prove
//! browser-readiness — what it shares with a browser, and what is actually
//! being tested, is that there are no threads.

use tampines_steam_tables::TampinesSteamArray;
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length, ThermodynamicTemperature, Time, Velocity};
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// Edwards pipe geometry, from `tests/edwards_blowdown.rs`.
const PIPE_LENGTH_M: f64 = 4.096;
const PIPE_ID_M: f64 = 0.073;
const P_INIT_PA: f64 = 7.0e6;
/// Uniform initial temperature, ~502 K — inside the subcooled-liquid region at
/// 7 MPa. The benchmark uses the Hendrie non-isothermal profile; a smoke test
/// does not need it, and a uniform value keeps this file short.
const T_INIT_K: f64 = 502.0;

/// Build the Edwards pipe with the benchmark's PISO configuration.
fn edwards_pipe(n_cells: i64, dt_us: f64) -> TampinesSteamArray {
    let area = std::f64::consts::PI * PIPE_ID_M * PIPE_ID_M / 4.0;
    let mut array = TampinesSteamArray::new(
        Length::new::<meter>(PIPE_LENGTH_M),
        Area::new::<square_meter>(area),
        n_cells,
        Time::new::<second>(dt_us * 1.0e-6),
    )
    .expect("valid Edwards pipe geometry");

    // Same PISO configuration as the benchmark: four outer correctors, four
    // inner pressure correctors, no under-relaxation.
    array.set_pimple_algorithm(4, 4, uom::si::f64::Ratio::new::<ratio>(1.0), uom::si::f64::Ratio::new::<ratio>(1.0));

    let n = array.mesh.n_cells;
    for c in 0..n {
        array.p.internal[c] = P_INIT_PA;
    }
    let temps: Vec<ThermodynamicTemperature> =
        (0..n).map(|_| ThermodynamicTemperature::new::<kelvin>(T_INIT_K)).collect();
    array
        .set_temperature_vector(temps)
        .expect("temperature vector length matches cell count");

    // Closed end at x = 0.
    array.set_inlet_velocity(Velocity::new::<meter_per_second>(0.0));
    array
}

#[test]
fn edwards_blowdown_advances_under_wasm() {
    // 8 cells / 50 us / 20 steps = 1 ms of transient. Enough for the
    // depressurisation wave to leave the break end and for every part of the
    // PIMPLE loop to have executed many times; small enough to finish quickly
    // in an unoptimised wasm build.
    let mut array = edwards_pipe(8, 50.0);
    let n = array.mesh.n_cells;

    for c in 0..n {
        assert!(
            (array.p.internal[c] - P_INIT_PA).abs() < 1.0,
            "cell {c}: initial field should be a uniform 7 MPa"
        );
    }

    // Open the break: the last cell sees ambient.
    for step in 0..20 {
        array.p.internal[n - 1] = 1.0e5;
        array.step();

        for c in 0..n {
            let p = array.p.internal[c];
            assert!(
                p.is_finite(),
                "step {step}, cell {c}: pressure went non-finite ({p}) — the solver diverged"
            );
            assert!(
                p > 0.0,
                "step {step}, cell {c}: pressure went non-positive ({p} Pa), which is unphysical"
            );
        }
    }

    // A blowdown must depressurise. This is a sanity floor, not an accuracy
    // claim — the benchmark test owns the accuracy.
    let mean_final: f64 = (0..n).map(|c| array.p.internal[c]).sum::<f64>() / n as f64;
    assert!(
        mean_final < P_INIT_PA,
        "after 1 ms of blowdown the mean pressure ({mean_final:.3e} Pa) should have fallen \
         below the initial 7 MPa; it did not, so the break is not doing anything"
    );
    assert!(
        mean_final > 1.0e5,
        "mean pressure ({mean_final:.3e} Pa) fell to or below ambient in 1 ms, which is far \
         too fast for a 4 m pipe — suspect a unit or timestep error rather than physics"
    );
}

#[test]
fn steam_property_flash_works_under_wasm() {
    // The IAPWS-IF97 backward flashes are the crate's hot path and are pure
    // arithmetic — no threads, no clock, no filesystem — so they should be
    // completely unaffected by the target. Cheap to assert, and it isolates a
    // property-library failure from a solver failure if the test above breaks.
    let array = edwards_pipe(4, 50.0);
    for c in 0..array.mesh.n_cells {
        let t = array.t.internal[c];
        assert!(t.is_finite() && t > 0.0, "cell {c}: flashed temperature {t} is not physical");
        let rho = array.rho.internal[c];
        assert!(
            (500.0..1200.0).contains(&rho),
            "cell {c}: subcooled water at 7 MPa/502 K should be ~800 kg/m3, got {rho}"
        );
    }
}
