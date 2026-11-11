// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK.

//! # The control-rod bank, cheaply: withdrawn is the record's core, and k falls as it goes in (gh:#1025)
//!
//! ## Methodology
//!
//! - **Model:** the 14 x 12 HTR-10 core of the 2026-10-07 k-vs-height record
//!   (`assemble_explicit_triso_with`, `Htr10CoreDesign::default()` with the
//!   bank at insertion f), data and materials at their defaults
//!   (`Htr10DataConfig::default()`: ENDF/B-VIII.0 at 300.15 K, URR and
//!   ~~DBRC~~ resonance scattering on, every bound law; `benchmark_default`;
//!   **CORRECTED 2026-11-11 (gh:#1333):** since 2026-11-10 the resonance
//!   sampler is RVS, not DBRC (gh:#1311), and since 2026-11-03 the graphite
//!   and SiC laws are LEAPR-generated, not the tapes (0c4f762ed); the
//!   2026-10-23 results were on DBRC and the tapes), `keff_vs_height::run_core`
//!   (the record's driver, bed majorant, source box and entropy mesh).
//! - **Points:** the bank withdrawn (`z_T` 119.2 cm), at B42's 282.618 cm
//!   (lower end ~54 cm into the 123.6 cm bed) and fully in (394.2 cm),
//!   ~~600 neutrons x [4 + 8] each~~ **1000 neutrons x [10 + 40] each
//!   since 2026-11-11 (gh:#1333)**, seed 20260917, every thread (the
//!   driver's k is bit-identical at 1, 2 and 4 threads, measured
//!   2026-11-11, so the thread count only sets the wall time).
//! - **Why these statistics (gh:#1333).** The full curve gives the two
//!   steps as Δk = 0.0695 (0.99449 -> 0.92497) and 0.0787 (-> 0.84624).
//!   At 600 x 8 active (4 800 histories) σ was 0.010-0.022 per arm, so the
//!   3σ bar (0.05-0.07) sat on top of the first step's expected fall and
//!   the test passed or failed by chance (it failed on 2026-11-10 at
//!   0.95762 ± 0.02174 -> 0.93275 ± 0.01313). The record's per-history
//!   spread (σ√N = 0.00258 x √200 000 ≈ 1.15, taken as 1.3 for margin)
//!   gives σ ≈ 0.0065 per arm at 40 000 active histories: a 3σ bar of
//!   ~0.028 against 0.0695, ~4.6σ of margin. 40 cycles also make σ itself
//!   a usable estimate (~±11 %, against ~±25 % from 8). 10 inactive
//!   cycles: the curve's traced runs (2000 per cycle) met their entropy
//!   criterion from cycle 1, so 10 is a margin; not re-checked at 1000
//!   per cycle.
//! - **Pass:** (1) the withdrawn k is within 5σ (this run's σ and the
//!   record's in quadrature) of the record's N = 12 point, 0.995125 ±
//!   0.001055 (`crates/nee_soon/verification_and_validation/htr10_seker_2026_10_07_10k/`);
//!   (2) k falls strictly with insertion, each step by more than 3σ
//!   combined; (3) no lost particle. Thresholds are the statistics', not the
//!   answer's: the full curve, at 2000 x [20 + 100] and more, is in
//!   `crates/dhoby-ghaut/verification_and_validation/htr10_rod_curve/`.
//!
//! ## Results
//!
//! See the test's doc comment.

use nee_soon::htr10_rmc::core_design::Htr10CoreDesign;
use nee_soon::htr10_rmc::core_model::assemble_explicit_triso_with;
use nee_soon::htr10_rmc::data::{load_htr10_nuclides, Htr10DataConfig, Htr10NuclideLayout};
use nee_soon::htr10_rmc::keff_vs_height::{bed_majorant, run_core, SweepStatistics, TEMPERATURE_K};
use nee_soon::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
use outram_mc_libs::run_diagnostics::RunDiagnostics;

/// The record's N = 12 point (ENDF/B-VIII.0, 2026-10-07).
const RECORD_N12: (f64, f64) = (0.995125, 0.001055);

/// See the module docs for the methodology.
///
/// # Results (2026-10-23, 4 threads, 644 s with the data)
///
/// Withdrawn 0.97237 ± 0.01000 (record 0.995125: −2.3σ combined, inside
/// the 5σ gate; at 7 200 histories the run is a harness check, not a
/// measurement); 282.618 cm 0.90813 ± 0.01370; fully in 0.84445 ± 0.01275.
/// Each step falls by 3.8σ and 3.4σ combined; 0 lost. Pass.
/// (600 x [4 + 8], tape laws, DBRC.)
///
/// # Results (2026-11-10, failure; gh:#1333)
///
/// After the two default changes (LEAPR laws, RVS) the same 600 x [4 + 8]
/// run gave 0.95762 ± 0.02174, 0.93275 ± 0.01313, 0.83678 ± 0.01244: the
/// first step fell 0.0249 against a 3σ bar of 0.0762. Root cause, measured
/// 2026-11-11: **the protocol could not resolve its own effect**. Not the
/// thread count (k bit-identical at 1, 2 and 4 threads), not a regression
/// (tape laws + DBRC reproduce the 2026-10-23 k bit for bit); the defaults
/// only redrew the random numbers. With σ ≈ 0.01-0.02 per arm the 3σ bar
/// (0.05-0.07) equalled the expected first step (0.0695 on the full curve),
/// so pass or fail was a coin toss; 2026-10-23 was the lucky side.
///
/// # Results (2026-11-11, 1000 x [10 + 40], 4 threads, 2300 s with the data on a contended 4-core host)
///
/// Withdrawn 0.99503 ± 0.00691 (record 0.995125: −0.01σ combined);
/// 282.618 cm 0.93430 ± 0.00625; fully in 0.84544 ± 0.00663 (50 000
/// histories each). Steps fall 0.0607 (6.5σ combined) and 0.0889 (9.8σ);
/// 0 lost. Pass. LEAPR laws and RVS, the current defaults.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "needs the reference ENDF data (HTR-10 tape set) and runs for about 40 minutes"
)]
fn the_bank_withdrawn_is_the_record_and_k_falls_as_it_goes_in() {
    let cfg = Htr10DataConfig::default();
    let Ok(layout) = Htr10NuclideLayout::plan(&cfg) else {
        eprintln!("SKIP: HTR-10 nuclear data could not be planned (reference data absent)");
        return;
    };
    let mut diag = RunDiagnostics::new("htr10 rod bank insertion");
    let nucs = match load_htr10_nuclides(&cfg, &layout, &mut diag) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("SKIP: HTR-10 nuclear data could not be loaded: {e}");
            return;
        }
    };
    let mats = htr10_material_set(&layout, Htr10MaterialConfig::benchmark_default(TEMPERATURE_K));
    let maj = bed_majorant(&mats, &nucs);
    let stats = SweepStatistics { name: "rod-test", particles: 1000, inactive: 10, active: 40, seed: 20_260_917 };
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut ks = Vec::new();
    for z in [119.2, 282.618, 394.2] {
        let d = Htr10CoreDesign::default().with_bank(Htr10CoreDesign::rod_insertion_for_lower_end_zt_cm(z));
        let core = assemble_explicit_triso_with(14, 12, 0, &d);
        let (p, _) = run_core(12, &core, &mats, &nucs, &maj, &stats, threads);
        eprintln!("  z_T {z:7.3} cm: k = {:.5} ± {:.5} ({} histories, {} lost)", p.k, p.sigma, p.histories, p.lost_locate);
        assert_eq!(p.lost_locate, 0, "lost particles at z_T {z}");
        ks.push((p.k, p.sigma));
    }
    let (k0, s0) = ks[0];
    let gap = (k0 - RECORD_N12.0).abs();
    let s = (s0 * s0 + RECORD_N12.1 * RECORD_N12.1).sqrt();
    assert!(gap < 5.0 * s, "withdrawn k {k0:.5} ± {s0:.5} is {:.1}σ from the record {:?}", gap / s, RECORD_N12);
    for w in ks.windows(2) {
        let (a, b) = (w[0], w[1]);
        let sc = (a.1 * a.1 + b.1 * b.1).sqrt();
        assert!(a.0 - b.0 > 3.0 * sc, "k did not fall by 3σ: {a:?} -> {b:?}");
    }
}
