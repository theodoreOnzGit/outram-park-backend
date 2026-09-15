//! **Price the discrete-inelastic angular distributions on Godiva** — a paired
//! ablation of ENDF MF=4/MT=51…90, bead `op-tm9f`.
//!
//! # What is being ablated
//!
//! Until this study, every inelastic collision in this crate drew
//! `mu_cm = 2*prn − 1` — isotropic in the centre of mass — while elastic used
//! the evaluation's full MF=4 tabulated cosine. The discrete inelastic levels
//! are **not** isotropic: ENDF/B-VIII.0 gives U-238 `mubar_cm` of `+0.033` at
//! 1 MeV rising to `+0.51` at 14 MeV on MT=51, and 39 of the 40 levels on each
//! of U-235 and U-238 carry anisotropic data.
//!
//! Sampling them isotropically understates `⟨μ⟩`, so the transport cross
//! section `Σ_tr = Σ_t(1 − ⟨μ⟩)` comes out too large, the diffusion coefficient
//! `D = 1/(3Σ_tr)` too small, leakage too low and `k` too high. Godiva is
//! **55.8 % leakage**, so it is maximally sensitive to exactly this.
//!
//! # Method
//!
//! Two arms over the **same seeds**, identical in every other respect —
//! geometry, the three ICSBEP nuclide densities, temperature, histories,
//! generation split, and the cross sections themselves (the ablation clears the
//! angular tables only, which `tests/inelastic_anisotropy_ablation_control.rs`
//! asserts):
//!
//! - **ANISO** — the level's own MF=4 CM cosine, via
//!   [`Nuclide::sample_inelastic_mu_cm`].
//! - **ISO** — [`Nuclide::with_isotropic_inelastic_scattering`], i.e. exactly
//!   the behaviour before `op-tm9f`.
//!
//! `N` seeds per arm (default 16, override with `OUTRAM_GODIVA_SEEDS`) at
//! 5000 histories × [40 inactive + 120 active]. A single run cannot resolve a
//! 100–200 pcm effect: one draw of this model carries seed-to-seed
//! `sd ≈ 180–205 pcm`, so two single runs differ by ~250 pcm from
//! re-randomisation alone. Both the unpaired and the **paired** difference are
//! reported — the arms share seeds, so if the pairing holds any correlation the
//! paired standard error is the smaller and more honest one; if it does not,
//! the paired `sd` will be the larger of the two and the unpaired figure is
//! what to quote. Read whichever the run actually shows rather than assuming.
//!
//! # Prediction, stated before the measurement
//!
//! Recorded on `op-tm9f` before this example existed: Godiva should move **DOWN
//! by roughly 180 pcm**, from ~+214 toward ~+30 against the ICSBEP benchmark,
//! priced three independent ways at +168 (ablation-scaled), +181 (measured as
//! the leakage share of the OpenMC offset) and +219 pcm (one-group diffusion
//! from the measured `P_NL`). If it moves *up*, or moves far more than ~220,
//! the diagnosis is wrong and should be revisited rather than tuned around.
//!
//! An independent check comes free: `k_inf` has no leakage, so it should be
//! **largely unchanged** by this. That is `examples/godiva_kinf_vs_openmc.rs`.
//!
//! # Results
//!
//! Filled in from a real run — see the V&V README in
//! `verification_and_validation/openmc_godiva_cross_code/`.
//!
//! ```text
//! OUTRAM_GODIVA_SEEDS=16 cargo run --release -p outram-mc-libs \
//!     --features endf-pebble-cases --example godiva_inelastic_anisotropy_ablation
//! ```

#[cfg(target_os = "android")]
fn main() {
    println!("godiva_inelastic_anisotropy_ablation is desktop-only (reads reference-data/endf/).");
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

    /// Mean, sample standard deviation and standard error of a sample.
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
            for (sd_chunk, out_chunk) in seeds.chunks(chunk).zip(out.chunks_mut(chunk)) {
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
                });
            }
        });
        out
    }

    pub fn run() {
        let n_seeds: usize = std::env::var("OUTRAM_GODIVA_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(16);

        println!("Reconstructing HEU isotopes from ENDF/B-VIII.0 (RECONR + BROADR @ 293.6 K)…");
        let t0 = Instant::now();
        let mut aniso = Vec::new();
        for &(file, name, _) in NUCLIDES {
            let Some(p) = reference_endf(file) else {
                println!(
                    "  missing {file} — set OUTRAM_PARK_ENDF_DIR or fetch the tape; skipping."
                );
                return;
            };
            aniso.push(
                Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
                    .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display())),
            );
        }
        let iso: Vec<Nuclide> = aniso
            .iter()
            .cloned()
            .map(Nuclide::with_isotropic_inelastic_scattering)
            .collect();
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
        let aniso = Arc::new(aniso);
        let iso = Arc::new(iso);

        println!("{n_seeds} seeds per arm, 5000 histories × [40 inactive + 120 active]…");
        let t = Instant::now();
        let a = run_arm(&aniso, &material, &seeds);
        println!("  ANISO arm done in {:.1} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let b = run_arm(&iso, &material, &seeds);
        println!("  ISO   arm done in {:.1} s\n", t.elapsed().as_secs_f64());

        let (ma, sa, ea) = stats(&a);
        let (mb, sb, eb) = stats(&b);
        let diff = ma - mb;
        let ediff = (ea * ea + eb * eb).sqrt();

        // Paired: the arms share seeds, so the per-seed difference may carry
        // less variance than either arm. It may also carry MORE, if the two
        // arms' RNG streams diverge at the first inelastic collision — which is
        // exactly what happens here, since the ablation changes how many draws
        // a history consumes. Report both and let the numbers say which holds.
        let d: Vec<f64> = a.iter().zip(&b).map(|(x, y)| x - y).collect();
        let (md, sdp, edp) = stats(&d);

        println!("Δk from ICSBEP HEU-MET-FAST-001 = 1.0000, pcm");
        println!("  arm       n     mean      sd     sem");
        println!(
            "  ANISO   {:>3}   {:+7.0}  {:>6.0}  {:>6.0}",
            a.len(),
            ma,
            sa,
            ea
        );
        println!(
            "  ISO     {:>3}   {:+7.0}  {:>6.0}  {:>6.0}",
            b.len(),
            mb,
            sb,
            eb
        );
        println!();
        println!(
            "  difference (ANISO − ISO), unpaired = {:+.0} ± {:.0} pcm  ({:.1} sigma)",
            diff,
            ediff,
            (diff / ediff).abs()
        );
        println!(
            "  difference (ANISO − ISO), paired   = {:+.0} ± {:.0} pcm  ({:.1} sigma), paired sd {:.0}",
            md,
            edp,
            (md / edp).abs(),
            sdp
        );
        if sdp > sa.max(sb) {
            println!(
                "  -> paired sd ({:.0}) EXCEEDS either arm's ({:.0}); the seeds do not pair, \
                 so quote the unpaired figure.",
                sdp,
                sa.max(sb)
            );
        } else {
            println!(
                "  -> paired sd ({:.0}) is below either arm's ({:.0}); the pairing helps, \
                 so quote the paired figure.",
                sdp,
                sa.max(sb)
            );
        }
        println!(
            "\n  Prediction on op-tm9f, recorded before this ran: ANISO − ISO ≈ −180 pcm \
             (priced at −168 / −181 / −219 three ways)."
        );
        println!(
            "  A single run of either arm cannot resolve that: one draw carries sd ≈ {:.0} pcm.",
            sa.max(sb)
        );
    }
}
