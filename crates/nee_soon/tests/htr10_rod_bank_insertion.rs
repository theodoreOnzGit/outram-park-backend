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
//!   (`Htr10DataConfig::default()`: ENDF/B-VIII.0 at 300.15 K, URR and DBRC
//!   on, every bound law; `benchmark_default`), `keff_vs_height::run_core`
//!   (the record's driver, bed majorant, source box and entropy mesh).
//! - **Points:** the bank withdrawn (`z_T` 119.2 cm), at B42's 282.618 cm
//!   (lower end ~54 cm into the 123.6 cm bed) and fully in (394.2 cm),
//!   600 neutrons x [4 + 8] each, seed 20260917, every thread.
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
//! See the test's doc comment (filled from its run).

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
/// # Results
///
/// PENDING (filled from the first run).
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "needs the reference ENDF data (HTR-10 tape set) and runs for about 15 minutes"
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
    let stats = SweepStatistics { name: "rod-test", particles: 600, inactive: 4, active: 8, seed: 20_260_917 };
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
