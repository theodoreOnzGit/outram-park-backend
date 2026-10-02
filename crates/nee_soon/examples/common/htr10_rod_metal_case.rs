// SPDX-License-Identifier: GPL-3.0-only
//! **The HTR-10 rod-metal case, shared by `htr10_rod_metal_full` and
//! `htr10_rod_metal_simplified`.**
//!
//! The two examples differ in ONE field, the rod-metal treatment
//! ([`RodMetalTreatment::Full`] or [`RodMetalTreatment::Simplified`]). Every
//! other setting is a literal here, so the pair is a controlled comparison by
//! construction and neither reads the environment. Each example pulls this in
//! with `#[path = "common/htr10_rod_metal_case.rs"] mod htr10_rod_metal_case;`;
//! a subdirectory without `main.rs` is not an example target, so Cargo does not
//! build this file on its own (the `outram-mc-libs/examples/common/` pattern).
//!
//! The nuclide set, materials and geometry are the library's
//! (`htr10_rmc::data`, `htr10_rmc::materials`, `assemble_explicit_triso`), the
//! same calls `htr10_rmc_keff` makes, so with the simplified knobs set that
//! driver builds the identical model.

#![allow(dead_code)]

use std::time::Instant;

use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;
use nee_soon::htr10_rmc::core_model::assemble_explicit_triso;
use nee_soon::htr10_rmc::keff_vs_height::{bed_majorant, fissile_entropy_mesh, fissile_source_box};
use nee_soon::htr10_rmc::data::{
    load_htr10_nuclides, Htr10DataConfig, Htr10DataError, Htr10NuclideLayout, RodMetalTreatment,
};
use nee_soon::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
use nee_soon::htr10_rmc::rmc_keff_at_ball_count;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::run_keff_csg_hybrid;
use outram_mc_libs::run_diagnostics::RunDiagnostics;

/// 27 °C, the temperature Li, Yu & Wei (2014) and Şeker & Çolak (2003) state.
pub const TEMP_K: f64 = 300.15;
/// Şeker layers N: 12 is the 123.576 cm critical-loading row.
pub const LAYERS: usize = 12;
/// Radial ring count of the bed tiling (as `htr10_endf8_height_sweep`).
pub const RINGS: usize = 14;
/// Histories per generation.
pub const PARTICLES: usize = 10_000;
/// Inactive generations.
pub const INACTIVE: usize = 30;
/// Active generations.
pub const ACTIVE: usize = 100;
/// RNG seed (single draw; seed-to-seed sd on this problem is ~180-210 pcm, so
/// pool seeds before quoting a difference between the two cases).
pub const SEED: u64 = 20_260_917;

/// Build and run the case with `rod_metal`; `name` labels the log and the
/// diagnostics record. Everything but `rod_metal` is the correct-physics
/// default ([`Htr10DataConfig::default`]): ENDF/B-VIII.0, natural carbon,
/// helium coolant, every bound thermal law.
pub fn run(name: &str, rod_metal: RodMetalTreatment) {
    println!("HTR-10 rod-metal case: {name}");
    println!("=========================================================");
    println!("  rod metal: {}", rod_metal.label());
    println!("  ENDF/B-VIII.0, natural carbon (C-12/C-13), helium coolant, all bound S(a,b)");
    println!(
        "  N = {LAYERS} layers, {RINGS} rings, {PARTICLES} x [{INACTIVE} + {ACTIVE}], seed {SEED}\n"
    );

    let data_cfg = Htr10DataConfig {
        rod_metal,
        temperature: ThermodynamicTemperature::new::<kelvin>(TEMP_K),
        ..Htr10DataConfig::default()
    };
    let layout =
        Htr10NuclideLayout::plan(&data_cfg).expect("the default data configuration is valid");
    let mut diag = RunDiagnostics::new(name);
    diag.note(format!("rod metal: {}", rod_metal.label()));
    eprintln!("Reconstructing cross sections:");
    let nucs = match load_htr10_nuclides(&data_cfg, &layout, &mut diag) {
        Ok(v) => v,
        Err(e @ Htr10DataError::BlockedByGh339) => {
            println!("REFUSED: {e}");
            return;
        }
        Err(e) => {
            println!("SKIP: {e}");
            return;
        }
    };

    let mats = htr10_material_set(&layout, Htr10MaterialConfig::benchmark_default(TEMP_K));
    for m in &mats {
        for c in &m.components {
            assert!(
                c.nuclide_idx < nucs.len(),
                "{} names an unloaded slot",
                m.name
            );
        }
    }
    let core = assemble_explicit_triso(RINGS, LAYERS, 0);
    println!(
        "  geometry: {} tiles, {} cells, {} universes",
        core.tiles, core.cells, core.universes
    );

    // Region-local majorant over the bed's materials (coolant included); the
    // reflector is surface-tracked.
    let maj = bed_majorant(&mats, &nucs);

    let settings = KeffSettings {
        n_particles: PARTICLES,
        n_inactive: INACTIVE,
        n_active: ACTIVE,
        temperature_k: TEMP_K,
        seed: SEED,
        compute: ComputeType::CpuMultiThread(ThreadCount::Auto),
        ..KeffSettings::default()
    };
    let src = fissile_source_box(&core);
    let entropy_mesh = fissile_entropy_mesh(&core);
    println!(
        "  nuclear data processed in {:.1} s ({} items)",
        diag.data_seconds(),
        diag.data_item_count()
    );

    let t = Instant::now();
    let res = diag.time_phase("transport (k-eigenvalue)", || {
        run_keff_csg_hybrid(
            &core.geometry,
            &mats,
            &nucs,
            std::slice::from_ref(&maj),
            Some(&entropy_mesh),
            src,
            &settings,
            None,
        )
    });
    let secs = t.elapsed().as_secs_f64();

    println!("\n  k_eff        = {:.6} +/- {:.6}", res.k_mean, res.k_std);
    match core
        .bed
        .as_ref()
        .and_then(|b| b.core_balls())
        .and_then(rmc_keff_at_ball_count)
    {
        Some(k) => println!(
            "  RMC (equal ball count) = {k:.6}, dk = {:+.0} pcm (sigma {:.0} pcm, single seed)",
            (res.k_mean - k) * 1.0e5,
            res.k_std * 1.0e5
        ),
        None => println!("  RMC reference: none at this ball count"),
    }
    println!("  lost locate  = {}", res.lost_locate);
    println!("  wall clock   = {secs:.1} s");
    if let (Some(first), Some(last)) = (res.entropy.first(), res.entropy.last()) {
        println!("  entropy      = {first:.4} -> {last:.4} bits");
    }
    diag.print_summary();
    diag.write_and_report();
}
