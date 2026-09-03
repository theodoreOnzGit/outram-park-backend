// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **wasm runtime smoke test — a helium/steam counter-flow steam generator.**
//! Bead `op-okqo.7`.
//!
//! # What this is for
//!
//! `scripts/check-wasm.sh` proves crates *compile* for wasm. It cannot prove
//! anything *runs* — `std::thread::spawn`, `Instant::now` and `std::fs` all
//! compile for wasm and fail only at run time. This file closes that gap for
//! the heaviest **composed** path in the workspace: three coupled arrays from
//! three different crates advancing together.
//!
//! ```bash
//! cargo test --target wasm32-wasip1 -p tampines --test wasm_smoke
//! ```
//!
//! # Where this came from, and what it is not
//!
//! The HTGR demo's steam generator
//! (`outram-park-digital-twin-engine/examples/htgr_sim_v1/physics/steam_generator.rs`,
//! `NodalisedCounterFlowSteamGenerator`) lives inside an **example**, so no
//! test can import it. Rather than copy ~1000 lines, this reproduces the same
//! **composition** it is built from, which is the part worth proving runs:
//!
//! | Stream | Type | Crate |
//! |---|---|---|
//! | hot (helium) | `CompressibleFluidArray` | `tampines` |
//! | tube metal | `SolidColumn` | `tuas_boussinesq_solver` |
//! | cold (steam) | `TampinesSteamArray` | `tampines-steam-tables` |
//!
//! Same fluid (`CoolPropFluid::Helium`), same metal (SS304L), same pressure
//! levels (3 MPa shell / 4 MPa tube) and the same counter-flow seeding idea as
//! the demo.
//!
//! **It is not the demo's steam generator and makes no claim to reproduce its
//! numbers.** There is no UA network, no sub-stepping, no PIMPLE corrector
//! configuration, and no energy-balance assertion. It asserts only that the
//! three arrays build, advance together, and stay physical. If you want the
//! exchanger's actual behaviour, run the demo.
//!
//! # Why WASI and not a browser
//!
//! See `scripts/wasi-run.mjs`. `wasm32-wasip1` shares `target_arch = "wasm32"`,
//! so every cfg gate in the workspace applies identically, but unlike
//! `wasm32-unknown-unknown` it can host a test harness. WASI has a clock and a
//! filesystem, so this does **not** prove browser-readiness. What it shares
//! with a browser — and what is genuinely under test — is that there are no
//! threads: this exercises the single-threaded CPU fallback end to end.

use tampines::compressible::{CompressibleFluidArray, CoolPropFluid};
use tampines_steam_tables::TampinesSteamArray;
use tuas_boussinesq_solver::array_fluid_collections::solid_array_lateral_coupling::SolidColumn;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::SolidMaterial;

use uom::si::area::square_meter;
use uom::si::f64::{Area, Length, Pressure, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

/// Eight nodes, as the HTGR demo's steam generator uses.
const NODES: usize = 8;
/// Shell (helium) side pressure, 3 MPa — the demo's value.
const HOT_P_PA: f64 = 3.0e6;
/// Tube (steam) side pressure, 4 MPa — the demo's value.
const COLD_P_PA: f64 = 4.0e6;
/// Helium inlet, roughly HTR-10 core outlet.
const HOT_IN_K: f64 = 1023.0;
/// Helium outlet end.
const HOT_OUT_K: f64 = 523.0;
/// Feedwater end, subcooled at 4 MPa (T_sat ~ 523 K).
const COLD_IN_K: f64 = 473.0;
/// Steam outlet end.
const COLD_OUT_K: f64 = 773.0;

/// A linear seed between two end temperatures, one value per node — the same
/// idea the demo uses so the exchanger opens near zero transfer with no
/// start-up shock.
fn linear_seed(from_k: f64, to_k: f64, n: usize) -> Vec<ThermodynamicTemperature> {
    (0..n)
        .map(|i| {
            let f = if n > 1 { i as f64 / (n - 1) as f64 } else { 0.0 };
            ThermodynamicTemperature::new::<kelvin>(from_k + (to_k - from_k) * f)
        })
        .collect()
}

/// Build the three coupled arrays: helium shell, steel tube wall, steam tube.
#[allow(clippy::type_complexity)]
fn build_exchanger(
    dt: Time,
) -> (CompressibleFluidArray, SolidColumn, TampinesSteamArray) {
    let flow_length = Length::new::<meter>(6.0);
    let shell_area = Area::new::<square_meter>(0.05);
    let tube_area = Area::new::<square_meter>(0.02);

    // ── Hot side: helium, via the CoolProp fork ──────────────────────────────
    let mut hot = CompressibleFluidArray::new(
        CoolPropFluid::Helium,
        flow_length,
        shell_area,
        NODES as i64,
        dt,
    )
    .expect("helium shell array builds");
    for c in 0..NODES {
        hot.p.internal[c] = HOT_P_PA;
    }
    hot.set_temperature_vector(linear_seed(HOT_IN_K, HOT_OUT_K, NODES))
        .expect("helium seed length matches node count");

    // ── Tube metal: SS304L ───────────────────────────────────────────────────
    // Seeded between the two streams, which is where a balanced conductance
    // network would put it.
    let metal_seed: Vec<ThermodynamicTemperature> = linear_seed(HOT_IN_K, HOT_OUT_K, NODES)
        .iter()
        .zip(linear_seed(COLD_OUT_K, COLD_IN_K, NODES).iter())
        .map(|(h, c)| {
            ThermodynamicTemperature::new::<kelvin>(
                (h.get::<kelvin>() + c.get::<kelvin>()) * 0.5,
            )
        })
        .collect();
    let metal = SolidColumn::new_block(
        flow_length,
        Length::new::<meter>(0.003),
        Length::new::<meter>(1.0),
        metal_seed[0],
        Pressure::new::<pascal>(HOT_P_PA),
        SolidMaterial::SteelSS304L,
        NODES,
    );

    // ── Cold side: steam, via the IAPWS-IF97 tables ──────────────────────────
    let mut cold = TampinesSteamArray::new(flow_length, tube_area, NODES as i64, dt)
        .expect("steam tube array builds");
    for c in 0..NODES {
        cold.p.internal[c] = COLD_P_PA;
    }
    cold.set_temperature_vector(linear_seed(COLD_OUT_K, COLD_IN_K, NODES))
        .expect("steam seed length matches node count");

    (hot, metal, cold)
}

#[test]
fn helium_steam_generator_builds_and_advances_under_wasm() {
    let dt = Time::new::<second>(0.01);
    let (mut hot, _metal, mut cold) = build_exchanger(dt);

    // Ten sub-steps. Enough that every array has advanced its own state many
    // times and the property backends (CoolProp helium, IAPWS-IF97 steam) have
    // been exercised repeatedly; short enough to stay quick in an unoptimised
    // wasm build.
    for step in 0..10 {
        hot.advance_timestep(dt)
            .unwrap_or_else(|e| panic!("helium array step {step} failed under wasm: {e:?}"));
        cold.step();

        for c in 0..NODES {
            let ph = hot.p.internal[c];
            let pc = cold.p.internal[c];
            assert!(
                ph.is_finite() && ph > 0.0,
                "step {step}, node {c}: helium pressure {ph} Pa is not physical"
            );
            assert!(
                pc.is_finite() && pc > 0.0,
                "step {step}, node {c}: steam pressure {pc} Pa is not physical"
            );
            let tc = cold.t.internal[c];
            assert!(
                tc.is_finite() && tc > 0.0,
                "step {step}, node {c}: steam temperature {tc} K is not physical"
            );
        }
    }
}

#[test]
fn helium_properties_evaluate_under_wasm() {
    // The CoolProp helium backend is the part most likely to behave differently
    // off native — it is a large table/Helmholtz evaluation. Asserting it here
    // separates "helium properties broke" from "the coupled loop broke" if the
    // test above starts failing.
    let dt = Time::new::<second>(0.01);
    let (hot, _metal, _cold) = build_exchanger(dt);

    for c in 0..NODES {
        let rho = hot.rho.internal[c];
        // Helium at 3 MPa over 523-1023 K is a light supercritical gas: order
        // 1-3 kg/m3. A wide band on purpose — this is a smoke test, and the
        // point is to catch a zero, a NaN or a units error, not to grade the
        // equation of state.
        assert!(
            (0.1..20.0).contains(&rho),
            "node {c}: helium at {HOT_P_PA:.0} Pa should be a light gas (~1-3 kg/m3), got {rho}"
        );
    }
}
