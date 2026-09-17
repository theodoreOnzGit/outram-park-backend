//! **HTR-10 core k-eff against the RMC benchmark** — `bn:op-867c`, gh #214.
//!
//! The epic's acceptance criterion. Explicit TRISO, hybrid delta/surface
//! tracking, reflector from IAEA-TECDOC-1382 Table 4-3.
//!
//! ```bash
//! cargo run --release -p nee_soon --example htr10_rmc_keff
//! OUTRAM_HTR10_HISTORIES=20000 OUTRAM_HTR10_RINGS=14 cargo run --release ...
//! ```
//!
//! # STATUS 2026-09-17: THIS DOES NOT WORK YET
//!
//! First run (8 rings x 12 layers, 1500 histories x [30+70]) returned
//!
//! ```text
//! k_eff        = 0.000000 +/- 0.000000
//! virtual coll = 2820163146          (18,801 per history)
//! entropy      = 0.0000 -> 0.0000
//! wall clock   = 80.0 s
//! ```
//!
//! Two findings, and they are separate:
//!
//! 1. **The majorant cost is as predicted, and is NOT the failure.** Measured by
//!    `examples/htr10_majorant_diagnosis.rs`: the bound is set by the UO2 kernel
//!    at 4.18 cm^-1 while the volume-weighted local total is ~0.18, giving
//!    `p_accept ~ 0.0432` — about **22 rejections per real collision**. That
//!    lands on the ~25x this crate measured independently for an undiluted
//!    kernel (`dh_universe.rs:101-108`). 18,801 per history is that 22x times a
//!    history's many collisions. Expensive, not fatal.
//!
//! 2. **k = 0 is a separate bug, NOT yet isolated.** Zero fission sites and zero
//!    entropy mean the run produced nothing at all, which 22x rejection does not
//!    explain by itself. Candidates not yet discriminated: the source box may
//!    not intersect the bed; the helium material is empty so `sigma_t = 0` and a
//!    flight through it can only ever reject; or `material_at` returns `None`
//!    inside the delta region and every flight reports `Exhausted`.
//!
//! **Do not treat this example as a result.** It runs end to end, which is
//! itself worth something — the geometry assembles, the hybrid dispatch engages,
//! the instrumentation reports — but it computes no eigenvalue.
//!
//! # Read this before quoting any number it prints
//!
//! - **ENDF/B-VIII.0**; RMC, MCNP, Serpent and HCP all used **VII.0**. On a
//!   graphite-moderated LEU system that difference alone is worth hundreds of
//!   pcm, so a disagreement CANNOT be attributed to transport.
//! - The reference quotes **no uncertainty** on any of its twelve values.
//! - The reflector densities are **R-Z homogenised**; TECDOC says a 3-D model
//!   must correct them for the boring geometries. Unadjusted, they smear the
//!   control-rod and helium-flow channels uniformly.
//! - **No control rods or absorber balls** are modelled.
//! - The realised TRISO count is **8340**, not 8335 — unattainable, see
//!   `cubic_array_in_ball`.

use std::time::Instant;

use nee_soon::htr10_rmc::core_model::{assemble_explicit_triso, mat};
use nee_soon::htr10_rmc::reflector::zone_composition;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading, Htr10Nuclides};
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::{run_keff_csg_hybrid, SourceBox};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::tally::mesh::RegularMesh;

const TEMP_K: f64 = 300.15;
const RMC_KEFF: f64 = 1.004288; // 123.576 cm loading height
const NUC: Htr10Nuclides = Htr10Nuclides {
    u235: 0, u238: 1, o16: 2, c_free: 3, c_graphite: 4, si28: 5, b10: 6,
};
/// Natural boron is 19.9 at.% B-10; the rest is effectively a non-absorber.
const B10_OF_NATURAL: f64 = 0.199;

fn env_usize(k: &str, d: usize) -> usize {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

fn nuclides() -> Option<Vec<Nuclide>> {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let load = |n: &str, f: &str| -> Option<Nuclide> {
        let p = base.join(f);
        p.exists().then_some(())?;
        eprint!("  {n:<6} ");
        let t = Instant::now();
        let r = Nuclide::from_endf_file(&p, n, TEMP_K, 1.0e-3).ok();
        eprintln!("{:.1?}", t.elapsed());
        r
    };
    let sab = ThermalScattering::from_endf_file(
        base.join("tsl-crystalline-graphite.endf").to_str()?, 30, TEMP_K, "c_Graphite",
    ).ok()?;
    Some(vec![
        load("U235", "n-092_U_235-ENDF8.0.endf")?,
        load("U238", "n-092_U_238.endf")?,
        load("O16",  "n-008_O_016-ENDF8.0.endf")?,
        load("C12",  "n-006_C_012-ENDF8.0.endf")?,
        load("C12",  "n-006_C_012-ENDF8.0.endf")?.with_thermal_scattering(sab),
        load("Si28", "n-014_Si_028-ENDF8.0.endf")?,
        load("B10",  "n-005_B_010-ENDF8.0.endf")?,
    ])
}

fn main() {
    let histories = env_usize("OUTRAM_HTR10_HISTORIES", 2000);
    let rings = env_usize("OUTRAM_HTR10_RINGS", 8);
    let layers = env_usize("OUTRAM_HTR10_LAYERS", 12);

    println!("HTR-10 core k-eff vs Li, Yu & Wei (2014), RMC {RMC_KEFF}");
    println!("=========================================================");
    println!("  ENDF/B-VIII.0 (references used VII.0 -- offset NOT corrected)");
    println!("  explicit TRISO, hybrid delta/surface tracking, TECDOC reflector\n");

    eprintln!("Reconstructing cross sections:");
    let Some(nucs) = nuclides() else {
        println!("SKIP: reference-data/endf/ not in this checkout.");
        return;
    };

    // Pebble materials, slots 0..5 in DhUniverse::pebble order.
    let mut mats = fuel_pebble_materials(NUC, BoronReading::Natural, TEMP_K);
    mats.truncate(6);
    // 6: helium -- deliberately near-void, as the paper's own model omits it.
    mats.push(Material { id: 70, name: "helium".into(), components: vec![], temperature: TEMP_K });
    // 7: reflector, TECDOC Table 4-3 zone 22 (graphite reflector structure).
    let z = zone_composition(22).expect("zone 22 is listed");
    mats.push(Material {
        id: 71,
        name: "reflector graphite (TECDOC zone 22)".into(),
        components: vec![
            NuclideComponent { nuclide_idx: NUC.c_graphite, atom_density: z.carbon },
            NuclideComponent { nuclide_idx: NUC.b10, atom_density: z.natural_boron * B10_OF_NATURAL },
        ],
        temperature: TEMP_K,
    });
    assert_eq!(mats.len(), mat::REFLECTOR + 1);

    // OUTRAM_HTR10_SURFACE=1 runs the SAME geometry with surface tracking only.
    let surface_only = std::env::var("OUTRAM_HTR10_SURFACE").is_ok();
    // OUTRAM_HTR10_HOMOG=1 drops the nested TRISO lattice for a homogenised
    // fuel zone. Not physical (the zone becomes pure kernel material) but it
    // DISCRIMINATES: if this fissions and the explicit model does not, the TRISO
    // lattice is the culprit.
    let homog = std::env::var("OUTRAM_HTR10_HOMOG").is_ok();
    let maj_idx = if surface_only { usize::MAX } else { 0 };
    let core = if homog {
        nee_soon::htr10_rmc::core_model::assemble(rings, layers, maj_idx)
    } else {
        assemble_explicit_triso(rings, layers, maj_idx)
    };
    println!("  fuel zone: {}", if homog { "HOMOGENISED" } else { "explicit TRISO lattice" });
    println!("  tracking: {}", if surface_only { "SURFACE ONLY" } else { "hybrid (delta bed)" });
    println!("  geometry: {} tiles, {} cells, {} universes",
             core.tiles, core.cells, core.universes);

    // Region-local majorant: the BED's materials only. The reflector is
    // surface-tracked, so it must NOT raise the bed's tracking cost.
    let grid: Vec<f64> = (0..4096)
        .map(|i| (1.0e-4_f64.ln() + (2.0e7_f64.ln() - 1.0e-4_f64.ln()) * i as f64 / 4095.0).exp())
        .collect();
    let bed_mats: Vec<usize> = (0..=mat::HELIUM).collect();
    let maj = Majorant::over_indices(&mats, &bed_mats, &nucs, &grid, 0.3);

    let settings = KeffSettings {
        n_particles: histories,
        n_inactive: 30,
        n_active: 70,
        temperature_k: TEMP_K,
        seed: 20260917,
        compute: ComputeType::CpuMultiThread(ThreadCount::Auto),
        ..KeffSettings::default()
    };
    let r = core.tiles as f64; let _ = r;
    let src = SourceBox {
        lower: Position::new(-50.0, -50.0, -50.0),
        upper: Position::new(50.0, 50.0, 50.0),
    };
    let entropy_mesh = RegularMesh {
        lower_left: [-60.0, -60.0, -60.0],
        upper_right: [60.0, 60.0, 60.0],
        dimension: [4, 4, 4],
    };

    println!("  {histories} histories x [{} inactive + {} active]\n",
             settings.n_inactive, settings.n_active);
    let t = Instant::now();
    let res = run_keff_csg_hybrid(
        &core.geometry, &mats, &nucs,
        if surface_only { &[] } else { std::slice::from_ref(&maj) },
        Some(&entropy_mesh), src, &settings, None,
    );
    let secs = t.elapsed().as_secs_f64();

    let pcm = (res.k_mean - RMC_KEFF) * 1.0e5;
    let sigma = res.k_std * 1.0e5;
    println!("  k_eff        = {:.6} +/- {:.6}", res.k_mean, res.k_std);
    println!("  RMC          = {RMC_KEFF:.6}");
    println!("  difference   = {pcm:+.0} pcm   (our sigma {sigma:.0} pcm)");
    println!("  virtual coll = {}", res.virtual_collisions);
    println!("  wall clock   = {secs:.1} s");
    println!("  generations reported: {}", res.k_by_generation.len());
    let nz = res.k_by_generation.iter().filter(|k| **k > 0.0).count();
    println!("  generations with k > 0: {nz}");
    for (i, k) in res.k_by_generation.iter().take(5).enumerate() {
        println!("    gen {i}: k = {k:.6}");
    }
    if let (Some(first), Some(last)) = (res.entropy.first(), res.entropy.last()) {
        println!("  entropy      = {first:.4} -> {last:.4} bits (ceiling {:.4})",
                 (entropy_mesh.n_bins() as f64).log2());
    }
    println!("\n  Gate is 500-1000 pcm. This is a REDUCED core ({rings} rings x {layers} layers),");
    println!("  not the 123.576 cm loading, and carries the VIII.0-vs-VII.0 offset.");
}
