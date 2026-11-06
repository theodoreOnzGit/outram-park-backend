// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK.

//! **HTR-10 `k_eff` against control-rod position** (gh:#1025).
//!
//! Orchestration only: the core is `assemble_explicit_triso_with` at a
//! [`Htr10CoreDesign`] whose rods sit where each case puts them, transported
//! by [`keff_vs_height::run_core`] (the 2026-10-07 record's driver,
//! `run_keff_csg_hybrid`, bed majorant, whole-fissile-region source box and
//! entropy mesh). Nuclear data, materials and the bed majorant are built ONCE
//! (they do not depend on the rods: the majorant covers the bed's materials
//! only, and the rods are surface-tracked in the reflector), then every case
//! is run in turn on the same data.
//!
//! Every physics default is on (`Htr10DataConfig::default()`: ENDF/B-VIII.0
//! at 300.15 K, URR and DBRC, every bound thermal law, real rod metal;
//! `Htr10MaterialConfig::benchmark_default`). Nothing here is an ablation.
//!
//! # Cases
//!
//! `OUTRAM_ROD_CASES` is a `;`-separated list of `LAYERS:ZT` items, `ZT`
//! the rods' lower-end position `z_T` \[cm\] (TECDOC's axial coordinate,
//! 119.2 withdrawn to 394.2 inserted). Default: the bank curve at 12 layers,
//! [`BANK_CURVE_ZT_CM`]. A case already present in the output CSV is
//! skipped, so a run is resumable.
//!
//! # Knobs
//!
//! `OUTRAM_HTR10_HISTORIES` (default 2000), `OUTRAM_HTR10_INACTIVE` (20),
//! `OUTRAM_HTR10_ACTIVE` (60), `OUTRAM_HTR10_SEED` (20260917),
//! `OUTRAM_HTR10_THREADS` (4), `OUTRAM_ROD_CSV` (output, default
//! `htr10_rod_curve.csv` in the working directory).
//!
//! Every case also appends its per-cycle `k` and Shannon entropy to
//! `<csv without .csv>_cycles.csv` (columns `layers,zt_cm,rod,cycle,active,k,entropy`;
//! `rod` 0 for the bank), so source convergence and the stationarity of the
//! active cycles can be checked, not assumed (added 2026-10-23 after an S3
//! run's first-to-last entropy moved 5.512 -> 5.462 bits).
//!
//! `OUTRAM_ROD_SINGLE=<1..10>` (gh:#1025): move only that rod (numbered as
//! `reflector_geometry::control_rod_azimuths_deg`), the other nine
//! withdrawn; unset, all ten move together (the bank). The `insertion`
//! column is the moving rod's, and `rod_lower_end_zt_cm` is empty for a
//! single rod (the rods are not at one position).
//!
//! `main` is CLI glue (exempt from the reaching-test rule); the pieces it
//! calls are tested where they live (`core_design`, `control_rod`,
//! `keff_vs_height`).

use std::io::Write;
use std::time::Instant;

use nee_soon::htr10_rmc::control_rod::BANK_CURVE_ZT_CM;
use nee_soon::htr10_rmc::core_design::Htr10CoreDesign;
use nee_soon::htr10_rmc::core_model::assemble_explicit_triso_with;
use nee_soon::htr10_rmc::data::{load_htr10_nuclides, Htr10DataConfig, Htr10NuclideLayout};
use nee_soon::htr10_rmc::keff_vs_height::{self, SweepStatistics, RINGS, TEMPERATURE_K};
use nee_soon::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
use outram_mc_libs::run_diagnostics::RunDiagnostics;

fn env_parse<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k).ok().and_then(|v| v.trim().parse().ok()).unwrap_or(d)
}

/// The cases: `(layers, z_T)`.
fn cases() -> Vec<(usize, f64)> {
    match std::env::var("OUTRAM_ROD_CASES") {
        Err(_) => BANK_CURVE_ZT_CM.iter().map(|&z| (12, z)).collect(),
        Ok(s) => s
            .split(';')
            .filter(|t| !t.trim().is_empty())
            .map(|t| {
                let (n, z) = t.split_once(':').unwrap_or_else(|| panic!("case {t:?} is not LAYERS:ZT"));
                (n.trim().parse().expect("layers"), z.trim().parse().expect("z_T"))
            })
            .collect(),
    }
}

/// The design for a case: rod insertion `f` on the bank, or on rod
/// `single` (0-based) alone.
fn design_for(f: f64, single: Option<usize>) -> Htr10CoreDesign {
    match single {
        None => Htr10CoreDesign::default().with_bank(f),
        Some(i) => Htr10CoreDesign::default().with_rod(i, f),
    }
}

fn main() {
    let single: Option<usize> = std::env::var("OUTRAM_ROD_SINGLE").ok().map(|v| {
        let n: usize = v.trim().parse().expect("OUTRAM_ROD_SINGLE is a rod number 1..10");
        assert!((1..=10).contains(&n), "OUTRAM_ROD_SINGLE is a rod number 1..10, got {n}");
        n - 1
    });
    let stats = SweepStatistics {
        name: "rod-curve",
        particles: env_parse("OUTRAM_HTR10_HISTORIES", 2000),
        inactive: env_parse("OUTRAM_HTR10_INACTIVE", 20),
        active: env_parse("OUTRAM_HTR10_ACTIVE", 60),
        seed: env_parse("OUTRAM_HTR10_SEED", 20_260_917u64),
    };
    let threads: usize = env_parse("OUTRAM_HTR10_THREADS", 4);
    let csv = std::env::var("OUTRAM_ROD_CSV").unwrap_or_else(|_| "htr10_rod_curve.csv".into());
    let done: Vec<(usize, f64)> = std::fs::read_to_string(&csv)
        .unwrap_or_default()
        .lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            Some((f.first()?.parse().ok()?, f.get(1)?.parse().ok()?))
        })
        .collect();
    let todo: Vec<(usize, f64)> = cases()
        .into_iter()
        .filter(|&(n, z)| !done.iter().any(|&(dn, dz): &(usize, f64)| dn == n && (dz - z).abs() < 1e-6))
        .collect();
    println!(
        "HTR-10 k_eff vs control-rod position (gh:#1025): {}",
        single.map_or("all ten rods together".to_string(), |i| format!("rod {} alone, nine withdrawn", i + 1))
    );
    println!(
        "  {} x [{} + {}], seed {}, {threads} threads, {} case(s) to run, {} already in {csv}",
        stats.particles, stats.inactive, stats.active, stats.seed, todo.len(), done.len()
    );
    if todo.is_empty() {
        return;
    }
    for &(n, z) in &todo {
        let f = Htr10CoreDesign::rod_insertion_for_lower_end_zt_cm(z);
        let d = design_for(f, single);
        if let Err(e) = d.check() {
            println!("REFUSED: case {n}:{z}: {e}");
            return;
        }
    }

    let mut diag = RunDiagnostics::new("htr10-rod-curve");
    let cfg = Htr10DataConfig::default();
    let layout = match Htr10NuclideLayout::plan(&cfg) {
        Ok(l) => l,
        Err(e) => {
            println!("REFUSED: {e}");
            return;
        }
    };
    let t = Instant::now();
    let nucs = match load_htr10_nuclides(&cfg, &layout, &mut diag) {
        Ok(v) => v,
        Err(e) => {
            println!("SKIP: {e}");
            return;
        }
    };
    let mats = htr10_material_set(&layout, Htr10MaterialConfig::benchmark_default(TEMPERATURE_K));
    let maj = keff_vs_height::bed_majorant(&mats, &nucs);
    println!("  data, materials and bed majorant: {:.1} s ({} nuclides)", t.elapsed().as_secs_f64(), nucs.len());

    let new_file = done.is_empty() && std::fs::metadata(&csv).is_err();
    let mut out = std::fs::OpenOptions::new().create(true).append(true).open(&csv).expect("open the output CSV");
    if new_file {
        writeln!(
            out,
            "layers,zt_cm,insertion,built_height_cm,balls,k,sigma,particles,inactive,active,seed,threads,histories,lost,entropy_first,entropy_last,transport_s,rod_lower_end_zt_cm"
        )
        .expect("write");
    }
    for (n, z) in todo {
        let design = design_for(Htr10CoreDesign::rod_insertion_for_lower_end_zt_cm(z), single);
        let moving = single.unwrap_or(0);
        let core = assemble_explicit_triso_with(RINGS, n, 0, &design);
        println!(
            "  case N = {n}, z_T = {z} cm (insertion {:.6}): {} cells, rods at z_T {:?}",
            design.rod_insertions[moving], core.cells, core.rod_lower_ends_zt_cm
        );
        let (p, res) = keff_vs_height::run_core(n, &core, &mats, &nucs, &maj, &stats, threads);
        {
            let path = csv.strip_suffix(".csv").unwrap_or(&csv).to_string() + "_cycles.csv";
            let fresh = std::fs::metadata(&path).is_err();
            let mut t = std::fs::OpenOptions::new().create(true).append(true).open(&path).expect("open the cycle trace");
            if fresh {
                writeln!(t, "layers,zt_cm,rod,cycle,active,k,entropy").expect("write");
            }
            let rod = single.map_or(0, |i| i + 1);
            for (c, k) in res.k_by_generation.iter().enumerate() {
                let e = res.entropy.get(c).copied().unwrap_or(f64::NAN);
                writeln!(t, "{n},{z},{rod},{},{},{k:.6},{e:.5}", c + 1, u8::from(c >= stats.inactive)).expect("write");
            }
        }
        let (e0, e1) = p.entropy_first_last.unwrap_or((f64::NAN, f64::NAN));
        let secs = p.transport_time.get::<uom::si::time::second>();
        println!(
            "  -> k = {:.5} +/- {:.5}  ({} histories, {} lost, entropy {e0:.3} -> {e1:.3} bits, {secs:.0} s)",
            p.k, p.sigma, p.histories, p.lost_locate
        );
        writeln!(
            out,
            "{n},{z},{:.9},{:.3},{},{:.6},{:.6},{},{},{},{},{threads},{},{},{e0:.4},{e1:.4},{secs:.1},{}",
            design.rod_insertions[moving],
            p.built_height_cm(),
            p.balls.map_or(String::new(), |b| b.to_string()),
            p.k,
            p.sigma,
            stats.particles,
            stats.inactive,
            stats.active,
            stats.seed,
            p.histories,
            p.lost_locate,
            core.rod_lower_end_zt_cm.map_or(String::new(), |v| format!("{v:.3}")),
        )
        .expect("write");
        out.flush().expect("flush");
    }
}
