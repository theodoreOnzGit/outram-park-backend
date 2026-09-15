//! **The Godiva maturity evidence, as a runnable ensemble** — many independent
//! seeds against ICSBEP HEU-MET-FAST-001, pooled.
//!
//! # Why this exists
//!
//! `examples/godiva_keff_endf_local.rs` runs **one seed**. One seed of this
//! model carries seed-to-seed `sd ≈ 180-200 pcm`, so it cannot resolve anything
//! finer than a few hundred pcm, and two runs of it differ by ~250 pcm from
//! re-randomisation alone. Every headline Godiva number in this crate's record
//! that was later revised was a single draw read as if it were the answer:
//! `+57 ± 173` became `+228 ± 18`, and a single `+512` became `+247 ± 43`.
//!
//! The pooled studies that did resolve those were run ad hoc and never
//! committed, so the crate's `CLAUDE.md` cited a 96-seed mean that nothing in
//! the repository could reproduce. This example is that harness, committed.
//!
//! # Methodology
//!
//! ICSBEP **HEU-MET-FAST-001** (Godiva): a bare sphere of HEU, `r = 8.7407 cm`,
//! at 293.6 K, with all **three** benchmark nuclides at their specified atom
//! densities (U-234 `4.9184e-4`, U-235 `4.4994e-2`, U-238 `2.4984e-3`
//! atoms/barn·cm). Benchmark `k_eff = 1.0000 ± 0.0010` — note the **±100 pcm**
//! experimental band, which no amount of seeds can see past.
//!
//! Data is reconstructed from the ENDF/B-VIII.0 tapes in `reference-data/endf/`
//! through this workspace's own NJOY port (RECONR + BROADR at 293.6 K) — not a
//! pre-built ACE library — so the number exercises the data chain as well as the
//! transport kernel.
//!
//! `N` independent seeds (default 32, override with `OUTRAM_GODIVA_SEEDS`) at
//! 5000 histories × [40 inactive + 120 active]. Reported: the pooled mean Δk
//! from the benchmark in pcm, the seed-to-seed sample `sd`, and the standard
//! error `sem = sd/√N` — **the sem is the uncertainty on the answer**; the `sd`
//! is what a single run would scatter by.
//!
//! Resolving power, so the seed count can be chosen rather than guessed at:
//!
//! | seeds | sem (at sd ≈ 190) |
//! |---|---|
//! | 16 | ±48 pcm |
//! | 32 | ±34 pcm |
//! | 64 | ±24 pcm |
//! | 128 | ±17 pcm |
//! | 256 | ±12 pcm |
//!
//! # Results
//!
//! See [`RECORDED_PCM`] for the measured value, when it was taken and what
//! changed to produce it. The V&V write-up is in
//! `verification_and_validation/openmc_godiva_cross_code/README.md`.
//!
//! ```text
//! OUTRAM_GODIVA_SEEDS=256 cargo run --release -p outram-mc-libs \
//!     --features endf-pebble-cases --example godiva_keff_ensemble
//! ```

#[cfg(target_os = "android")]
fn main() {
    println!("godiva_keff_ensemble is desktop-only (reads reference-data/endf/).");
}

#[cfg(not(target_os = "android"))]
fn main() {
    desktop::run();
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use outram_mc_libs::material::nuclide::Nuclide;
    use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Instant;

    const TEMP_K: f64 = 293.6;
    const RADIUS_CM: f64 = 8.7407;
    const WORKERS: usize = 4;

    const NUCLIDES: &[(&str, &str, f64)] = &[
        ("n-092_U_234-ENDF8.0.endf", "U234", 4.9184e-4),
        ("n-092_U_235-ENDF8.0.endf", "U235", 4.4994e-2),
        ("n-092_U_238.endf", "U238", 2.4984e-3),
    ];

    pub fn run() {
        let n_seeds: usize = std::env::var("OUTRAM_GODIVA_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(32);

        println!("Reconstructing HEU isotopes from ENDF/B-VIII.0 (RECONR + BROADR @ 293.6 K)…");
        let t0 = Instant::now();
        let mut nuclides = Vec::new();
        for &(file, name, _) in NUCLIDES {
            let Some(p) = reference_endf(file) else {
                println!(
                    "  missing {file} — set OUTRAM_PARK_ENDF_DIR or fetch the tape; skipping."
                );
                return;
            };
            nuclides.push(
                Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
                    .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display())),
            );
        }
        println!(
            "Nuclear data ready in {:.1} s.\n",
            t0.elapsed().as_secs_f64()
        );

        let material = Material {
            id: 1,
            name: "Godiva HEU".into(),
            temperature: TEMP_K,
            components: NUCLIDES
                .iter()
                .enumerate()
                .map(|(i, &(_, _, rho))| NuclideComponent {
                    nuclide_idx: i,
                    atom_density: rho,
                })
                .collect(),
        };

        let seeds: Vec<u64> = (1..=n_seeds as u64).collect();
        let nuclides = Arc::new(nuclides);
        let done = Arc::new(AtomicUsize::new(0));

        println!("{n_seeds} seeds, 5000 histories × [40 inactive + 120 active]…");
        let t = Instant::now();
        let mut pcm = vec![0.0f64; seeds.len()];
        let chunk = seeds.len().div_ceil(WORKERS);
        std::thread::scope(|s| {
            for (sd_chunk, out_chunk) in seeds.chunks(chunk).zip(pcm.chunks_mut(chunk)) {
                let nuclides = Arc::clone(&nuclides);
                let material = &material;
                let done = Arc::clone(&done);
                s.spawn(move || {
                    for (k, &seed) in sd_chunk.iter().enumerate() {
                        let settings = KeffSettings {
                            n_particles: 5000,
                            n_inactive: 40,
                            n_active: 120,
                            temperature_k: TEMP_K,
                            seed,
                            ..KeffSettings::default()
                        };
                        let r = run_keff(RADIUS_CM, material, &nuclides, &settings);
                        out_chunk[k] = (r.k_mean - 1.0) * 1.0e5;
                        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                        if n % 16 == 0 {
                            println!("  {n} seeds done ({:.0} s)", t.elapsed().as_secs_f64());
                        }
                    }
                });
            }
        });
        println!("  all seeds done in {:.1} s\n", t.elapsed().as_secs_f64());

        let n = pcm.len() as f64;
        let mean = pcm.iter().sum::<f64>() / n;
        let sd = (pcm.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0)).sqrt();
        let sem = sd / n.sqrt();

        println!("Godiva vs ICSBEP HEU-MET-FAST-001 (benchmark k = 1.0000 ± 0.0010):");
        println!("  seeds         {}", pcm.len());
        println!("  mean          {mean:+.0} pcm");
        println!("  seed-to-seed  sd  {sd:.0} pcm   (what ONE run scatters by)");
        println!("  uncertainty   sem ±{sem:.0} pcm   (on this pooled mean)");
        println!("  distance from benchmark: {:.1} sem", (mean / sem).abs());
        println!(
            "  inside the experiment's own ±100 pcm band: {}",
            if mean.abs() <= 100.0 { "YES" } else { "no" }
        );
        println!("\n  Previously recorded: {RECORDED_PCM:+.0} pcm (see this file's docs).");
        println!(
            "  Move from that baseline: {:+.0} pcm.",
            mean - RECORDED_PCM
        );
    }

    /// The pooled Godiva offset from ICSBEP HEU-MET-FAST-001, in pcm, as last
    /// measured by this example.
    ///
    /// **`+214 pcm` (64-seed paired ensemble, 2026-09-13)** was the value before
    /// the discrete-inelastic angular distributions were wired in; it is kept
    /// here as the baseline the current number is compared against, and is
    /// superseded by whatever this example's own run reports. See
    /// `examples/godiva_keff_endf_local.rs` for the history of how this number
    /// moved: `+341` → `+57` (single draws, both unresolvable) → `+314`
    /// (96 seeds, after the MT=91 Q-value cap) → `+214` (after reading the
    /// evaluated MF=6 continuum law).
    pub const RECORDED_PCM: f64 = 214.0;
}
