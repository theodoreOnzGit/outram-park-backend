// SPDX-License-Identifier: GPL-3.0

//! **Runtime gate: route 4's nuclide construction RUNS in a wasm runtime
//! and gives the native numbers** (epic #925's wasm criterion, gh:#931).
//!
//! # Methodology
//!
//! - **Input:** ENDF/B-VIII.0 U-234, compiled in with `include_bytes!` so
//!   nothing reads a file: bytes in, values out. Fissile with an unresolved
//!   range, so PURR's tables (and HEATR before them) are exercised.
//! - **Path:** `Nuclide::process_evaluation` at 293.6 K, tolerance 1e-3
//!   (njoy's `process` since gh:#931), then
//!   `ProcessedEvaluation::to_f64s` → `from_f64s` (the browser pool's hand-off)
//!   → `Nuclide::from_processed`.
//! - **Fingerprint:** FNV-1a over the bits of every word of the product, and
//!   of the nuclide's `xs_at_energy` (all nine fields) at 200 log-spaced
//!   energies from 1e-5 eV to 20 MeV and 50 energies inside the URR range.
//!   Pass: equal to the native fingerprint pinned below.
//! - **Runs:** natively (`cargo test --release`) and under
//!   `wasm32-wasip1` (`cargo test --release --target wasm32-wasip1 -p
//!   outram-mc-libs --test wasm_route4_runtime --no-run`, then
//!   `node --no-warnings scripts/wasi-run.mjs <file>.wasm` from
//!   `crates/njoy-outram-park-fork`, as its `wasm_process_runtime.rs` says
//!   for a nested worktree). That runtime has no threads, like a browser; it
//!   has a clock and files, unlike one. The headless-Chromium check was not
//!   run for this change.
//!
//! # Results
//!
//! Recorded in `verification_and_validation/route4_process_931/README.md`.

use petir::mathf::RealMath;
use njoy_outram_park_fork::endf::tape::Tape;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::processed::ProcessedEvaluation;

const U234: &[u8] = include_bytes!("../../../reference-data/endf/n-092_U_234-ENDF8.0.endf");

/// The fingerprint the native run printed (x86_64 Linux, release,
/// 2026-10-17).
const NATIVE_FINGERPRINT: u64 = 0x871c_dd4d_51d2_06f2;

/// FNV-1a over 64-bit words.
fn fnv(words: impl Iterator<Item = u64>, mut h: u64) -> u64 {
    for w in words {
        for b in w.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

#[test]
fn route4_runs_in_wasm_and_gives_the_native_numbers() {
    let tape = Tape::read(U234).expect("parse U-234");
    let mat = tape.materials()[0];
    let p = Nuclide::process_evaluation(&tape, mat, 293.6, 1.0e-3, 1.0e-3).expect("route 4");
    let words = p.to_f64s();
    let mut h = fnv(words.iter().map(|x| x.to_bits()), 0xcbf2_9ce4_8422_2325);
    let back = ProcessedEvaluation::from_f64s(&words).expect("decode");
    let n = Nuclide::from_processed(&tape, mat, "U234", 293.6, back).expect("nuclide");
    let (lo, hi) = n.urr_range_ev().expect("U-234 has URR tables");
    let mut energies: Vec<f64> = (0..200)
        .map(|k| 1.0e-5 * (2.0e7_f64 / 1.0e-5).r_powf(f64::from(k) / 199.0))
        .collect();
    energies.extend((1..=50).map(|k| lo + (hi - lo) * f64::from(k) / 51.0));
    for e in energies {
        let x = n.xs_at_energy(e, 293.6);
        h = fnv(
            [
                x.total, x.elastic, x.fission, x.absorption, x.inelastic, x.n2n, x.n3n, x.mt5,
                x.nu_fission,
            ]
            .iter()
            .map(|v| v.to_bits()),
            h,
        );
    }
    println!(
        "U-234 route 4 at 293.6 K: fingerprint {h:#018x}, {} product words, target {}",
        words.len(),
        std::env::consts::ARCH
    );
    assert_eq!(h, NATIVE_FINGERPRINT, "this runtime's numbers differ from the native run's");
}
