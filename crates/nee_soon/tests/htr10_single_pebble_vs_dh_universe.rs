// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See the module header of
// `nee_soon::htr10_rmc` for licence terms.

//! # The CSG single pebble against independent code (gh:#849, #1707)
//!
//! The CSG pebble is built from the core's own TRISO and pebble builders.
//! This checks that build against a separate model of the same pebble:
//! `outram_mc_libs::dh_universe::DhUniverse::pebble` with
//! `PebbleParams::htr10_li2014`, code that shares nothing with the CSG
//! pebble but the nuclide and material tables.
//!
//! ## Like for like since 2026-11-22 (gh:#1707)
//!
//! ~~The CSG arm was [`Htr10Cell::single_pebble`]: 8335 particles on the
//! core's cubic lattice in a reflective 6 cm cube, against DhUniverse's
//! random packing in a reflective sphere of equal volume. "What differs, by
//! construction: the particle positions and the reflective boundary.
//! Neither is expected to be resolved at this budget."~~ **CHANGED
//! 2026-11-22 (gh:#1707, maintainer: "Test clearly isn't like for like and
//! shld be altered to make it like for like").** Those two differences are
//! resolved, and they are not small. At 10000 × \[50 + 200\] (Shannon
//! entropy converged within the 50 inactive batches, seed 20261013, 4-core
//! Xeon @ 2.10 GHz, CSG 3829 s, DH 356 s) the old comparison gave CSG
//! 1.68276 ± 0.00057 against DH 1.68732 ± 0.00059, **CSG − DH = −455 ± 82
//! pcm (−5.5σ)**. The 2026-10-13 pass (+102 ± 347 pcm) and the later
//! failures at the default budget (e.g. −856 ± 265 pcm) are both consistent
//! with that: the default budget could not resolve a 455 pcm systematic,
//! so pass or fail depended on the seed's draw. The test did not compare
//! like with like, so a failure could not say whether either code was
//! wrong. Neither the 3σ gate nor the seed was moved.
//!
//! The CSG arm is now [`Htr10Cell::single_pebble_like_dh_universe`]: the
//! **same particle centres** (`dh_universe::pack_in_ball`, with DH's radii,
//! fraction and seed, which is exactly the call `DhUniverse::pebble` makes)
//! in the **same reflective sphere**. The two arms differ only in the code
//! that tracks them. The −455 pcm is then the effect of lattice against
//! random packing plus cube against sphere, and separating the two is not
//! this test's job. The geometry is drawn in
//! `verification_and_validation/htr10_mgxs_cells/single_pebble_like_dh_*.png`.
//!
//! ## Methodology
//!
//! - **Data:** `htr10_rmc::data` at its defaults (ENDF/B-VIII.0 at 300.15 K,
//!   graphite S(a,b), natural boron in the graphite), one nuclide table for
//!   both arms; materials `htr10_material_set(..., benchmark_default(300.15))`.
//!   The DH table is the CSG's own materials in DH's order:
//!   `[kernel, buffer, IPyC, SiC, OPyC, matrix, shell, coolant] =
//!   mat::[KERNEL, BUFFER, IPYC, SIC, OPYC, GRAPHITE, GRAPHITE, HELIUM]`.
//! - **Shared geometry:** `PebbleParams::htr10_li2014()` (the default
//!   design's TRISO radii, 2.5 cm fuel zone, 3 cm pebble, 5.0248 % packing,
//!   which RSA realises as 8345 particles), helium out to
//!   `r = (3 * 216 / (4 pi))^(1/3) = 3.7221 cm` (the volume of the old
//!   6 cm cube, kept so the moderation ratio is the one on record),
//!   reflective there.
//! - **CSG arm:** that packing wrapped in a 10³ tracking lattice over the
//!   fuel zone (`create_triso_lattice`), delta-tracked with the core's
//!   `bed_majorant`, `run_keff_csg_hybrid`.
//! - **DH arm:** `DhUniverse::pebble(params, DhTreatment::DeltaTracking)`.
//! - **Same settings** for both: 4000 × \[10 + 50\] by default
//!   (`OUTRAM_CELL_PARTICLES` / `_INACTIVE` / `_ACTIVE` override), 300.15 K,
//!   seed 20261013, all threads.
//! - **Prediction, recorded 2026-11-22 before the first like-for-like run:**
//!   `dk` consistent with zero, `|dk| < 3σ`, at the default budget and at
//!   10000 × \[50 + 200\]. A residual of a few hundred pcm resolved at high
//!   statistics would be a defect in one of the two trackers.
//! - **Pass (gh:#849):** `|dk| < 3 sigma_combined`, with each arm's own
//!   `k_std` (single seed; the autocorrelation-corrected sigma is #850's).
//!   No history may be lost in the CSG cell.
//!
//! ## Results
//!
//! **2026-11-22, like for like**, seed 20261013, 4-core Intel Xeon @
//! 2.30 GHz, 4 threads, 15.7 GiB RAM:
//!
//! | budget | CSG pebble k (8345 TRISO, DH packing) | CSG time | DhUniverse pebble k | DH time | whole test (incl. data loading) | CSG − DH | verdict |
//! |---|---|---|---|---|---|---|---|
//! | 4000 × \[10 + 50\] (default) | 1.68625 ± 0.00185 (0 lost) | 415 s | 1.68850 ± 0.00197 | 52 s | 808 s (13.5 min; ~341 s loading) | −225 ± 271 pcm (−0.83σ) | passes |
//! | 10000 × \[50 + 200\] | 1.68863 ± 0.00056 (0 lost) | 4026 s | 1.68732 ± 0.00059 | 313 s | 4669 s (77.8 min; ~330 s loading) | +131 ± 81 pcm (+1.62σ) | passes |
//!
//! The prediction holds at both budgets: on the same geometry the two
//! trackers agree within 3σ, and at high statistics a tracker defect larger
//! than about 250 pcm (3σ) is excluded. The +1.6σ is not resolved; a
//! second seed or batch-means σ (#850) would say whether it is noise.
//! The DH arm repeats bit for bit the 1.68732 ± 0.00059 of the earlier
//! 10000 × \[50 + 200\] run (same packing, seed and settings), a
//! reproducibility check on it. Source convergence: 50 inactive batches
//! of 10000 brought the Shannon entropy to its plateau in the earlier
//! high-statistics run on this pebble; this run did not print the entropy,
//! so that was not re-checked here.
//!
//! ~~**2026-10-13 (old, not like for like), 4000 × \[10 + 50\]:** CSG
//! single pebble 1.68619 ± 0.00223 (0 lost), DhUniverse 1.68517 ± 0.00266,
//! CSG − DH +102 ± 347 pcm (+0.29σ), passes.~~ Superseded: that comparison
//! carried the −455 ± 82 pcm systematic above, which the budget could not
//! see. The run took 494 s (CSG) and 66 s (DH), 755 s for the whole test
//! with about 3 min of data loading.

// Needs nuclear data files or file output: `std` only (gh:#1640).
#![cfg(feature = "std")]
use petir::mathf::RealMath;
use nee_soon::htr10_mgxs::cells::Htr10Cell;
use nee_soon::htr10_rmc::core_model::mat;
use nee_soon::htr10_rmc::data::{load_htr10_nuclides, Htr10DataConfig, Htr10NuclideLayout};
use nee_soon::htr10_rmc::keff_vs_height::bed_majorant;
use nee_soon::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
use outram_mc_libs::dh_universe::{DhTreatment, DhUniverse, PebbleParams};
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::run_keff_csg_hybrid;
use outram_mc_libs::run_diagnostics::RunDiagnostics;
use uom::si::thermodynamic_temperature::kelvin;

fn env_usize(k: &str, d: usize) -> usize {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

/// See the module docs for the methodology.
///
/// # Results
///
/// See the module docs and
/// `verification_and_validation/htr10_mgxs_cells/README.md`.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "needs the reference ENDF data (HTR-10 tape set) and runs for minutes"
)]
fn csg_single_pebble_matches_the_dh_universe_pebble() {
    let cfg = Htr10DataConfig::default();
    let t_k = cfg.temperature.get::<kelvin>();
    let Ok(layout) = Htr10NuclideLayout::plan(&cfg) else {
        eprintln!("SKIP: HTR-10 nuclear data could not be planned (reference data absent)");
        return;
    };
    let mut diag = RunDiagnostics::new("htr10 single pebble vs DhUniverse");
    let nucs = match load_htr10_nuclides(&cfg, &layout, &mut diag) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("SKIP: HTR-10 nuclear data could not be loaded: {e}");
            return;
        }
    };
    let mats = htr10_material_set(&layout, Htr10MaterialConfig::benchmark_default(t_k));
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let settings = KeffSettings {
        n_particles: env_usize("OUTRAM_CELL_PARTICLES", 4_000),
        n_inactive: env_usize("OUTRAM_CELL_INACTIVE", 10),
        n_active: env_usize("OUTRAM_CELL_ACTIVE", 50),
        temperature_k: t_k,
        seed: 20_261_013,
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)),
        ..KeffSettings::default()
    };

    // The shared pebble: DH's parameters with the CSG's materials in DH's
    // order, helium to the old 6 cm cube's equal-volume sphere.
    let order = [
        mat::KERNEL,
        mat::BUFFER,
        mat::IPYC,
        mat::SIC,
        mat::OPYC,
        mat::GRAPHITE,
        mat::GRAPHITE,
        mat::HELIUM,
    ];
    let r_eq = (3.0 * 216.0 / (4.0 * std::f64::consts::PI)).r_cbrt();
    let params = PebbleParams::htr10_li2014()
        .with_materials(order.iter().map(|&m| mats[m].clone()).collect())
        .with_coolant(r_eq);

    // CSG arm: the same packing in the same sphere.
    let cell = Htr10Cell::single_pebble_like_dh_universe(&params, 0);
    let majorant = bed_majorant(&mats, &nucs);
    let t = std::time::Instant::now();
    let csg = run_keff_csg_hybrid(
        &cell.geometry,
        &mats,
        &nucs,
        std::slice::from_ref(&majorant),
        Some(&cell.entropy_mesh()),
        cell.source_box(),
        &settings,
        None,
    );
    let t_csg = t.elapsed().as_secs_f64();

    // DH arm.
    let dh = DhUniverse::pebble(params, DhTreatment::DeltaTracking).expect("DH pebble builds");
    let t = std::time::Instant::now();
    let dhk = dh.keff(&nucs, &settings);
    let t_dh = t.elapsed().as_secs_f64();

    let dk = csg.k_mean - dhk.k_mean;
    let sig = (csg.k_std.r_powi(2) + dhk.k_std.r_powi(2)).sqrt();
    println!(
        "{} x [{} + {}], {threads} threads, {}",
        settings.n_particles,
        settings.n_inactive,
        settings.n_active,
        diag.hardware().headline()
    );
    println!(
        "CSG pebble ({} TRISO, DH packing) k = {:.5} ± {:.5} ({t_csg:.0} s, {} lost); DhUniverse pebble (r_eq {r_eq:.4} cm) k = {:.5} ± {:.5} ({t_dh:.0} s)",
        cell.triso_particles, csg.k_mean, csg.k_std, csg.lost_locate, dhk.k_mean, dhk.k_std
    );
    println!(
        "CSG - DH = {:+.0} ± {:.0} pcm ({:+.2} sigma)",
        dk * 1e5,
        sig * 1e5,
        dk / sig
    );
    assert_eq!(csg.lost_locate, 0, "no history lost in the CSG cell");
    assert!(
        dk.abs() < 3.0 * sig,
        "CSG pebble {:.5} and DhUniverse {:.5} differ by {:.1} sigma",
        csg.k_mean,
        dhk.k_mean,
        dk / sig
    );
}
