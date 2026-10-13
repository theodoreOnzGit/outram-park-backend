// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See the module header of
// `nee_soon::htr10_rmc` for licence terms.

//! # The CSG single pebble against independent code (gh:#849)
//!
//! [`Htr10Cell::single_pebble`] is built from the core's own TRISO lattice
//! and pebble builders. This checks that build against a separate model of
//! the same pebble: `outram_mc_libs::dh_universe::DhUniverse::pebble` with
//! `PebbleParams::htr10_li2014`, which places its particles by random
//! packing in code that shares nothing with the CSG lattice.
//!
//! ## Methodology
//!
//! - **Data:** `htr10_rmc::data` at its defaults (ENDF/B-VIII.0 at 300.15 K,
//!   graphite S(a,b), natural boron in the graphite), one nuclide table for
//!   both arms; materials `htr10_material_set(..., benchmark_default(300.15))`.
//!   The DH table is the CSG's own materials in DH's order:
//!   `[kernel, buffer, IPyC, SiC, OPyC, matrix, shell, coolant] =
//!   mat::[KERNEL, BUFFER, IPYC, SIC, OPYC, GRAPHITE, GRAPHITE, HELIUM]`.
//! - **CSG arm:** the single pebble (one 6 cm ball, 8335 particles on the
//!   core's cubic lattice, helium) in a reflective 6 cm cube, delta-tracked
//!   with the core's `bed_majorant`, `run_keff_csg_hybrid`.
//! - **DH arm:** `PebbleParams::htr10_li2014()` (same TRISO radii, 2.5 cm
//!   zone, 3 cm pebble, 5.0248 % packing = 8335 particles, randomly packed),
//!   with a helium shell to the **equal-volume sphere** of the cube,
//!   `r = (3 * 216 / (4 pi))^(1/3) = 3.7221 cm`, reflective there, delta
//!   tracking (`DhTreatment::DeltaTracking`).
//! - **Same settings** for both: particles × \[inactive + active\] as below,
//!   300.15 K, seed 20261013, all threads.
//! - **What differs, by construction:** the particle positions (cubic
//!   lattice against random packing) and the reflective boundary (cube
//!   against sphere, around a near-transparent helium gap). Neither is
//!   expected to be resolved at this budget.
//! - **Prediction, recorded before the first run:** `|dk| < 300 pcm`, sign
//!   not predicted.
//! - **Pass (gh:#849):** `|dk| < 3 sigma_combined`, with each arm's own
//!   `k_std` (single seed; the autocorrelation-corrected sigma is #850's).
//!
//! ## Results
//!
//! See the test's own doc comment.

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
/// Recorded in `verification_and_validation/htr10_mgxs_cells/README.md`.
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
        n_particles: env_usize("OUTRAM_CELL_PARTICLES", 10_000),
        n_inactive: env_usize("OUTRAM_CELL_INACTIVE", 20),
        n_active: env_usize("OUTRAM_CELL_ACTIVE", 100),
        temperature_k: t_k,
        seed: 20_261_013,
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)),
        ..KeffSettings::default()
    };

    // CSG arm.
    let cell = Htr10Cell::single_pebble();
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

    // DH arm: the same materials in DH's order, helium to the cube's
    // equal-volume sphere.
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
    let r_eq = (3.0 * cell.volume_cm3 / (4.0 * std::f64::consts::PI)).cbrt();
    let params = PebbleParams::htr10_li2014()
        .with_materials(order.iter().map(|&m| mats[m].clone()).collect())
        .with_coolant(r_eq);
    let dh = DhUniverse::pebble(params, DhTreatment::DeltaTracking).expect("DH pebble builds");
    let t = std::time::Instant::now();
    let dhk = dh.keff(&nucs, &settings);
    let t_dh = t.elapsed().as_secs_f64();

    let dk = csg.k_mean - dhk.k_mean;
    let sig = (csg.k_std.powi(2) + dhk.k_std.powi(2)).sqrt();
    println!(
        "{} x [{} + {}], {threads} threads, {}",
        settings.n_particles,
        settings.n_inactive,
        settings.n_active,
        diag.hardware().headline()
    );
    println!(
        "CSG single pebble k = {:.5} ± {:.5} ({t_csg:.0} s, {} lost); DhUniverse pebble (r_eq {r_eq:.4} cm) k = {:.5} ± {:.5} ({t_dh:.0} s)",
        csg.k_mean, csg.k_std, csg.lost_locate, dhk.k_mean, dhk.k_std
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
        "CSG single pebble {:.5} and DhUniverse {:.5} differ by {:.1} sigma",
        csg.k_mean,
        dhk.k_mean,
        dk / sig
    );
}
