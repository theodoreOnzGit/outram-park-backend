//! **HTTR start-up core `k_eff`**: explicit TRISO, delta tracking in the
//! compacts, ENDF/B-VIII.0, 300 K, uncalibrated (nothing tuned to any
//! reference). Model: `nee_soon::httr`.
//!
//! ```bash
//! cargo run --release -p nee_soon --example httr_keff
//! OUTRAM_HTTR_HISTORIES=20000 OUTRAM_HTTR_INACTIVE=50 OUTRAM_HTTR_ACTIVE=200 \
//!   OUTRAM_HTTR_THREADS=14 cargo run --release -p nee_soon --example httr_keff
//! ```
//!
//! Knobs (each defaults to the benchmark model):
//! - `OUTRAM_HTTR_STATE=critical|subcritical` (rods as R1 §3.1.2.11-12);
//! - `OUTRAM_HTTR_HISTORIES`, `_INACTIVE`, `_ACTIVE`, `_SEED`, `_THREADS`;
//! - `OUTRAM_HTTR_SEEDS=n`: n independent seeds (seed, seed + 1, ...),
//!   pooled; the sem quoted is the pooled one;
//! - ablations: `OUTRAM_HTTR_GRAPHITE_TSL=crystalline|10P|30P`,
//!   `OUTRAM_HTTR_NO_SAB=1` (free gas), `OUTRAM_HTTR_SURFACE=1` (no delta
//!   tracking), `OUTRAM_HTTR_IMPURITY_B10_ONLY=1` (R1's B-10-only
//!   impurities).
//!
//! It prints the result against the IRPhEP benchmark (validation) and R1's
//! MCNP5 results (verification, code to code). Results and their hardware
//! are recorded in
//! `crates/nee_soon/verification_and_validation/httr_startup_core/README.md`.
//!
//! Driver glue (`main`): the model it runs is tested by `nee_soon::httr::tests`.

use petir::mathf::RealMath;
use std::time::Instant;

use nee_soon::htr10_rmc::materials::GraphiteLaw;
use nee_soon::httr::{
    assemble_httr, compact_majorant, fuel_entropy_mesh, fuel_source_box, httr_material_set,
    load_httr_nuclides, CompactTracking, HttrDataConfig, HttrMaterialConfig, HttrThermal,
    RodPositions, BENCHMARK_CRITICAL, BENCHMARK_SUBCRITICAL, BENCHMARK_TEMPERATURE_K, MCNP5_R1,
};
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::run_keff_csg_hybrid;
use outram_mc_libs::run_diagnostics::RunDiagnostics;

fn env<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

fn main() {
    let on = |k: &str| std::env::var(k).is_ok();
    let subcritical = std::env::var("OUTRAM_HTTR_STATE").as_deref() == Ok("subcritical");
    let rods = if subcritical {
        RodPositions::SUBCRITICAL
    } else {
        RodPositions::CRITICAL
    };
    let reference = if subcritical {
        BENCHMARK_SUBCRITICAL
    } else {
        BENCHMARK_CRITICAL
    };
    let graphite_law = match std::env::var("OUTRAM_HTTR_GRAPHITE_TSL").ok() {
        None => GraphiteLaw::default(),
        Some(v) => GraphiteLaw::from_name(&v).expect("crystalline, 10P or 30P"),
    };
    let thermal = if on("OUTRAM_HTTR_NO_SAB") {
        HttrThermal::FreeGas
    } else {
        HttrThermal::Bound
    };
    let tracking = if on("OUTRAM_HTTR_SURFACE") {
        CompactTracking::Surface
    } else {
        CompactTracking::Delta
    };
    let mut mcfg = HttrMaterialConfig::benchmark(BENCHMARK_TEMPERATURE_K);
    if on("OUTRAM_HTTR_IMPURITY_B10_ONLY") {
        mcfg = mcfg.without_impurity_b11();
    }
    let histories: usize = env("OUTRAM_HTTR_HISTORIES", 5000);
    let inactive: usize = env("OUTRAM_HTTR_INACTIVE", 50);
    let active: usize = env("OUTRAM_HTTR_ACTIVE", 100);
    let seed0: u64 = env("OUTRAM_HTTR_SEED", 20261016);
    let n_seeds: u64 = env("OUTRAM_HTTR_SEEDS", 1);
    let threads = std::env::var("OUTRAM_HTTR_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map_or(ThreadCount::Auto, ThreadCount::Fixed);

    println!("HTTR start-up core, {} state", if subcritical { "SUBCRITICAL" } else { "CRITICAL" });
    println!("  rods (bottom above fuel bottom, cm): {rods:?}");
    println!(
        "  graphite S(a,b) {graphite_law:?}, thermal {thermal:?}, tracking {tracking:?}, \
         impurity B-11 {}",
        mcfg.impurity_b11
    );
    let mut diag = RunDiagnostics::new("httr-keff");
    diag.note(format!(
        "{histories} histories x [{inactive} + {active}], {n_seeds} seed(s) from {seed0}, \
         state {}, graphite {graphite_law:?}, thermal {thermal:?}, tracking {tracking:?}, \
         impurity B-11 {}",
        if subcritical { "subcritical" } else { "critical" },
        mcfg.impurity_b11
    ));
    let data_cfg = HttrDataConfig {
        graphite_law,
        thermal,
        ..HttrDataConfig::default()
    };
    let nucs = match load_httr_nuclides(&data_cfg, &mut diag, |_| {}) {
        Ok(n) => n,
        Err(e) => {
            println!("SKIP: {e}");
            return;
        }
    };
    println!("  nuclear data processed in {:.1} s", diag.data_seconds());
    let mats = httr_material_set(mcfg);
    let core = assemble_httr(rods, tracking);
    println!(
        "  geometry: {} cells, {} universes, {} lattices; {} fuel rods x {} particles",
        core.geometry.cells.len(),
        core.geometry.universes.len(),
        core.geometry.lattices.len(),
        core.fuel_rods,
        core.particles_per_rod
    );
    let t_maj = Instant::now();
    let maj = compact_majorant(&mats, &nucs);
    println!("  compact majorant: {} nodes ({:.1} s)", maj.len(), t_maj.elapsed().as_secs_f64());
    let mesh = fuel_entropy_mesh();
    let mut ks = Vec::new();
    for s in 0..n_seeds {
        let settings = KeffSettings {
            n_particles: histories,
            n_inactive: inactive,
            n_active: active,
            temperature_k: BENCHMARK_TEMPERATURE_K,
            seed: seed0 + s,
            compute: ComputeType::CpuMultiThread(threads),
            ..KeffSettings::default()
        };
        let t = Instant::now();
        let r = diag.time_phase("transport (k-eigenvalue)", || {
            run_keff_csg_hybrid(
                &core.geometry,
                &mats,
                &nucs,
                std::slice::from_ref(&maj),
                Some(&mesh),
                fuel_source_box(),
                &settings,
                None,
            )
        });
        let h_last = r.entropy.last().copied().unwrap_or(f64::NAN);
        println!(
            "  seed {}: k = {:.5} +/- {:.5}  ({:.0} s, final entropy {:.3} bits)",
            settings.seed,
            r.k_mean,
            r.k_std,
            t.elapsed().as_secs_f64(),
            h_last
        );
        // Source convergence, measured: the entropy trace every tenth of the
        // run, and the active k split in halves (a drifting source shows
        // as a difference between them).
        let ent: Vec<String> = (1..=10)
            .filter_map(|q| r.entropy.get(q * r.entropy.len() / 10 - 1))
            .map(|h| format!("{h:.3}"))
            .collect();
        println!("    entropy at each tenth of the run [bits]: {}", ent.join(" "));
        let act = &r.k_by_generation[inactive.min(r.k_by_generation.len())..];
        if act.len() >= 4 {
            let half = act.len() / 2;
            let m = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;
            let (a, b) = (m(&act[..half]), m(&act[half..]));
            let sd = |s: &[f64], mu: f64| {
                (s.iter().map(|x| (x - mu).r_powi(2)).sum::<f64>() / ((s.len() - 1) as f64 * s.len() as f64)).sqrt()
            };
            println!(
                "    active halves: {a:.5} +/- {:.5} then {b:.5} +/- {:.5} (difference {:+.0} pcm)",
                sd(&act[..half], a),
                sd(&act[half..], b),
                (b - a) * 1e5
            );
        }
        diag.note(format!("seed {}: k = {:.6} +/- {:.6}; entropy tenths {}", settings.seed, r.k_mean, r.k_std, ent.join(" ")));
        ks.push((r.k_mean, r.k_std));
    }
    let n = ks.len() as f64;
    let mean = ks.iter().map(|k| k.0).sum::<f64>() / n;
    let sem = if ks.len() > 1 {
        let var = ks.iter().map(|k| (k.0 - mean).r_powi(2)).sum::<f64>() / (n - 1.0);
        (var / n).sqrt()
    } else {
        ks[0].1
    };
    println!("\n  k_eff = {mean:.5} +/- {sem:.5} ({} seed(s))", ks.len());
    let d = mean - reference.k;
    let band = if d >= 0.0 { reference.plus } else { reference.minus };
    println!(
        "  VALIDATION vs {}: {:.4} (-{:.4}/+{:.4}): {:+.0} pcm ({:+.2} benchmark sigma)",
        reference.label,
        reference.k,
        reference.minus,
        reference.plus,
        d * 1e5,
        d / band
    );
    println!("  VERIFICATION vs R1's MCNP5 (+/-0.0001), ENDF/B-VIII.0 here against each library there:");
    for (lib, kc, ks_) in MCNP5_R1 {
        let r = if subcritical { ks_ } else { kc };
        println!("    {lib:<28} {r:.4}: {:+.0} pcm", (mean - r) * 1e5);
    }
    println!("  hardware: {}", diag.hardware().headline());
    diag.note(format!("k_eff = {mean:.6} +/- {sem:.6}"));
    diag.print_summary();
    diag.write_and_report();
}
