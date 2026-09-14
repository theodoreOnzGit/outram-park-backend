//! **V&V — what reading the evaluated MF=6 continuum law is worth on Godiva,
//! priced by a paired seed ensemble.**
//!
//! # Why a whole ensemble for one switch
//!
//! Godiva's seed-to-seed spread at this program's settings is **sd ≈ 205 pcm**
//! (measured 2026-09-13; see `crates/outram-mc-libs/CLAUDE.md` and gh:#196).
//! A single run therefore cannot resolve anything smaller than ~400 pcm, and the
//! repo has already been bitten once by reading a single draw as the code's
//! answer — the `+57 ± 173 pcm` that was really a −0.97 sigma draw of a
//! `+228 ± 18 pcm` distribution. So this program runs **both arms over the same
//! set of independent seeds** and reports pooled means with their standard
//! errors, not one number each.
//!
//! # Methodology
//!
//! Two arms, identical in every respect but one switch:
//!
//! | arm | MT=91 and MT=16 outgoing energy |
//! |---|---|
//! | `MF6` | the evaluation's own `f₀(E→E')` (ENDF MF=6 LAW=1), sampled with the same `ContinuousTabular` sampler the MF=5 fission spectrum uses |
//! | `WEISSKOPF` | the Weisskopf evaporation stand-in, via [`Nuclide::without_evaluated_continuum`] — what every case used before 2026-09-13 |
//!
//! Each arm runs `N` seeds (default 64, override with `OUTRAM_GODIVA_SEEDS`) at
//! the settings of `examples/godiva_keff_endf_local.rs` — 5000 histories ×
//! [40 inactive + 120 active], all three ICSBEP nuclides, ENDF/B-VIII.0 from
//! `reference-data/endf/`, **single-thread** transport so each run is
//! bit-identical to what that example would produce at that seed. Seeds are
//! spread across worker threads; the transport itself stays deterministic.
//!
//! The arms are **not** paired in the variance-reduction sense: switching the
//! law changes how many draws the sampler consumes, which decorrelates the two
//! streams completely. The reported difference therefore carries
//! `sem_diff = √(sem_A² + sem_B²)`, with no pairing benefit assumed.
//!
//! # Results (2026-09-13, 64 seeds per arm, ENDF/B-VIII.0)
//!
//! ```text
//!   arm         n     mean      sd     sem
//!   MF6        64      +214     160      20
//!   WEISSKOPF  64      +319     204      25
//!   difference (MF6 − WEISSKOPF) = -105 ± 32 pcm  (3.3 sigma)
//! ```
//!
//! **Reading the evaluated law is worth −105 ± 32 pcm on Godiva, 3.3 sigma —
//! toward the benchmark.** Godiva's HIGH-tier bias drops from `+319 ± 25` to
//! `+214 ± 20 pcm`.
//!
//! Two things are worth stating about this number rather than just recording it:
//!
//! - **The `WEISSKOPF` arm reproduces the independently recorded baseline.**
//!   `+319 ± 25 pcm` here against `+314 ± 21 pcm` from the separate 96-seed
//!   study of the same pre-MF=6 code (gh:#196) — 0.15 sigma apart. The two
//!   ensembles share no code beyond the transport kernel itself, so that
//!   agreement is a check on this harness, not a restatement of it.
//! - **The sign was predicted wrong before the run, and the prediction is left
//!   on the record.** The evaluated law is *softer* than the stand-in where
//!   MT=91 opens (`⟨E'/E⟩` 0.2095 against 0.2787 at 2 MeV on U-238), and the
//!   stated expectation was that more energy lost per collision would shorten
//!   the mean free path, cut leakage, and push *k* **up**. It went down. In a
//!   bare fast metal sphere the spectrum-hardness terms — `ν̄(E)`, and U-238's
//!   threshold fission above ~1 MeV — evidently dominate the leakage term. That
//!   decomposition is **not** measured here; it is the hypothesis this result
//!   leaves open, not a finding.
//!
//! The switch also carries a second change, and the arms cannot separate the
//! two: in the `MF6` arm the second (n,2n) neutron is an **independent draw**
//! from the evaluated law, where the `WEISSKOPF` arm duplicates the primary's
//! outgoing state (the pre-2026-09-13 behaviour, gh:#192 item 2). Both are part
//! of "reading MF=6", so the −105 pcm is their combined worth. Godiva's MT=16
//! rate is small next to MT=91's 10–25 % of collisions, so the continuum term
//! is expected to dominate — expected, not measured.
//!
//! This program deliberately carries **no pass/fail gate**: it is an instrument
//! for pricing a mechanism, and a gate on a number whose whole point is to be
//! measured would be circular. The gate on Godiva's absolute agreement lives in
//! `examples/godiva_keff_endf_local.rs`.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example godiva_mf6_continuum_ensemble
//! ```

#[cfg(target_os = "android")]
fn main() {
    println!("godiva_mf6_continuum_ensemble is desktop-only (reads reference-data/endf/).");
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
    use std::sync::Arc;
    use std::time::Instant;

    /// Godiva's fuel temperature \[K\] — the tapes are broadened to it.
    const TEMP_K: f64 = 293.6;
    /// ICSBEP HEU-MET-FAST-001 sphere radius \[cm\].
    const RADIUS_CM: f64 = 8.7407;
    /// Worker threads. Each runs whole seeds; transport inside a seed is serial.
    const WORKERS: usize = 4;

    /// `(tape file, nuclide name, atom density \[atoms/barn·cm\])` — the three
    /// ICSBEP nuclides, same numbers as `godiva_keff_endf_local`.
    const NUCLIDES: &[(&str, &str, f64)] = &[
        ("n-092_U_234-ENDF8.0.endf", "U234", 4.9184e-4),
        ("n-092_U_235-ENDF8.0.endf", "U235", 4.4994e-2),
        ("n-092_U_238.endf", "U238", 2.4984e-3),
    ];

    /// Mean, sample standard deviation and standard error of a sample, in pcm.
    fn stats(x: &[f64]) -> (f64, f64, f64) {
        let n = x.len() as f64;
        let mean = x.iter().sum::<f64>() / n;
        let var = x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
        let sd = var.sqrt();
        (mean, sd, sd / n.sqrt())
    }

    /// Run one arm over `seeds`, returning Δk from the benchmark in pcm per seed.
    fn run_arm(nuclides: &Arc<Vec<Nuclide>>, material: &Material, seeds: &[u64]) -> Vec<f64> {
        let mut out = vec![0.0; seeds.len()];
        let chunk = seeds.len().div_ceil(WORKERS);
        std::thread::scope(|s| {
            for (wi, (sd_chunk, out_chunk)) in seeds
                .chunks(chunk)
                .zip(out.chunks_mut(chunk))
                .enumerate()
            {
                let nuclides = Arc::clone(nuclides);
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
                    }
                    let _ = wi;
                });
            }
        });
        out
    }

    pub fn run() {
        let n_seeds: usize = std::env::var("OUTRAM_GODIVA_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(64);

        println!("Reconstructing HEU isotopes from ENDF/B-VIII.0 (RECONR + BROADR @ 293.6 K)…");
        let t0 = Instant::now();
        let mut with_mf6 = Vec::new();
        for &(file, name, _) in NUCLIDES {
            let Some(p) = reference_endf(file) else {
                println!("  missing {file} — set OUTRAM_PARK_ENDF_DIR or fetch the tape; skipping.");
                return;
            };
            let n = Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
                .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
            println!(
                "  {name}: MF=6 continuum law {}",
                if n.has_evaluated_continuum() {
                    "present"
                } else {
                    "ABSENT (falls back to Weisskopf in both arms)"
                }
            );
            with_mf6.push(n);
        }
        let without_mf6: Vec<Nuclide> = with_mf6
            .iter()
            .cloned()
            .map(Nuclide::without_evaluated_continuum)
            .collect();
        println!("Nuclear data ready in {:.1} s.\n", t0.elapsed().as_secs_f64());

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
        let with_mf6 = Arc::new(with_mf6);
        let without_mf6 = Arc::new(without_mf6);

        println!("{n_seeds} seeds per arm, 5000 histories × [40 inactive + 120 active]…");
        let t = Instant::now();
        let a = run_arm(&with_mf6, &material, &seeds);
        println!("  MF6 arm done in {:.1} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let b = run_arm(&without_mf6, &material, &seeds);
        println!("  WEISSKOPF arm done in {:.1} s\n", t.elapsed().as_secs_f64());

        let (ma, sa, ea) = stats(&a);
        let (mb, sb, eb) = stats(&b);
        let diff = ma - mb;
        let ediff = (ea * ea + eb * eb).sqrt();

        println!("Δk from ICSBEP HEU-MET-FAST-001 = 1.0000, pcm");
        println!("  arm         n     mean      sd     sem");
        println!("  MF6       {:>3}   {:+7.0}  {:>6.0}  {:>6.0}", a.len(), ma, sa, ea);
        println!("  WEISSKOPF {:>3}   {:+7.0}  {:>6.0}  {:>6.0}", b.len(), mb, sb, eb);
        println!(
            "  difference (MF6 − WEISSKOPF) = {:+.0} ± {:.0} pcm  ({:.1} sigma)",
            diff,
            ediff,
            (diff / ediff).abs()
        );
        println!(
            "\n  A single run of either arm cannot resolve this: one draw carries \
             sd ≈ {:.0} pcm.",
            sa.max(sb)
        );
    }
}
