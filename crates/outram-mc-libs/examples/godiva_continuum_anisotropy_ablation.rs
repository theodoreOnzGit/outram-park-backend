//! **Price the MF=6 continuum angular law on Godiva** — a paired ablation of
//! ENDF MF=6 LAW=1's `f₁ … f_NA` coefficients on MT=91 and MT=16, bead
//! `op-og56`.
//!
//! # What is being ablated
//!
//! Until `op-og56`, `njoy-outram-park-fork`'s MF=6 parser read ENDF `NA` only to
//! compute the row stride, kept the energy density `f₀`, and discarded
//! `f₁ … f_NA`; `LANG` was never read. So every continuum-inelastic (MT=91) and
//! (n,2n) (MT=16) neutron left the collision isotropically in the frame the law
//! names. The evaluations say otherwise: 8652 of U-238's 8654 MT=91 rows carry
//! `NA > 0`, and 5294 of U-235's 5296.
//!
//! This is the **third and last** of the angular channels. Elastic (MF=4/MT=2)
//! was always sampled; the discrete inelastic levels (MF=4/MT=51…90) were wired
//! in by `op-tm9f` and priced at **−198 pcm** by
//! `examples/godiva_inelastic_anisotropy_ablation.rs`; the continuum is this
//! one.
//!
//! # Method
//!
//! Two arms over the **same seeds**, identical in every other respect —
//! geometry, the three ICSBEP nuclide densities, temperature, histories,
//! generation split, and the cross sections themselves. The ablation replaces
//! each branch's angular law with
//! [`ContinuumAngular::Ablated`](njoy_outram_park_fork::nuclear_data::secondary::ContinuumAngular::Ablated)
//! and touches nothing else — not `f₀(E→E')`, not the branch yields, not a
//! single cross section. `tests/continuum_angular_ablation_control.rs` asserts
//! that, including that the two arms consume identical RNG variates in identical
//! order.
//!
//! - **ANISO** — the evaluated per-row Legendre cosine law.
//! - **ISO** — [`Nuclide::with_isotropic_continuum_scattering`], i.e. exactly
//!   the behaviour before `op-og56`.
//!
//! `N` seeds per arm (default 16, override with `OUTRAM_GODIVA_SEEDS`) at
//! 5000 histories × [40 inactive + 120 active]. A single run cannot resolve an
//! effect of this size: one draw of this model carries seed-to-seed
//! `sd ≈ 173 pcm`, so two single runs differ by ~245 pcm from re-randomisation
//! alone. Both the unpaired and the paired difference are printed; read
//! whichever the run actually shows to be tighter rather than assuming.
//!
//! # Prediction, stated before the measurement
//!
//! **Expect a SMALL move DOWN — well under 50 pcm, plausibly under 20.**
//!
//! The direction is the same mechanism as `op-tm9f`: forward-peaked emission
//! raises `⟨μ⟩`, lowers `Σ_tr = Σ_t(1 − ⟨μ⟩)`, lengthens the transport mean free
//! path and so increases leakage. Godiva is 55.8 % leakage, so `k` falls.
//!
//! The *magnitude* is what separates this from `op-tm9f`, and it is why the
//! prediction is small rather than another ~200 pcm. Weighted by each row's own
//! emission probability `f₀` — the `⟨μ_cm⟩` a neutron actually experiences —
//! U-238's MT=91 law is **exactly isotropic near threshold** and only turns on
//! at several MeV (measured 2026-09-16):
//!
//! | `E_in` | pdf-weighted `⟨μ_cm⟩` |
//! |---|---|
//! | 0.4356 MeV (threshold) | 0.000000 |
//! | 1.02 MeV | 0.000000 |
//! | 3.00 MeV | +0.001684 |
//! | 8.50 MeV | +0.073219 |
//! | 14.0 MeV | +0.272349 |
//! | 20.0 MeV | +0.387079 |
//!
//! The discrete levels of `op-tm9f` are anisotropic from ~1 MeV — the bulk of a
//! fission spectrum. This law has structure only above ~8 MeV, where a fission
//! spectrum has a percent or two of its flux. The headline "peak `|⟨μ⟩|` =
//! 0.557" quoted from the raw coefficients is **not** the number to reason
//! from: it sits in the far tail of a high-energy table where `f₀` is
//! negligible.
//!
//! **If this comes back at ~200 pcm, the diagnosis is wrong** and the wiring
//! should be suspected before the physics — most likely the angular law being
//! applied at the wrong incident energy, which would import the 14 MeV cosines
//! into the 2 MeV flux.
//!
//! # A second effect: PREDICTED HARDER, MEASURED (weakly) SOFTER
//!
//! For a centre-of-mass law the lab energy is
//! `E' = E_cm + E_trans + 2·μ_cm·√(E_cm·E_trans)`, so a forward-peaked cosine
//! raises `⟨E'_lab⟩` per collision. That argument was recorded here as
//! "this hardens the spectrum and moves `op-os8x` the wrong way".
//!
//! **Measured 2026-09-16** (`examples/godiva_continuum_spectrum_ablation.rs`,
//! 8 seeds per arm), ANISO against ISO:
//!
//! | measure | relative change | |
//! |---|---|---|
//! | mean `E` | **−0.076 % ± 0.109** | 0.7 σ |
//! | mean `ln E` | **−0.008 % ± 0.008** | 1.0 σ |
//! | flux fraction below 300 keV | **+0.157 % ± 0.226** | 0.7 σ |
//!
//! **Nothing is resolved at 2 σ, and all three central values point the other
//! way** — softer, not harder. The prediction is therefore *not supported*; it
//! is also not refuted, and the honest statement is a bound: the spectral
//! effect of this law is smaller than about `0.22 %` in mean `E`, against the
//! `+0.45 %` residual `op-os8x` is about. **It cannot be the explanation for
//! `op-os8x`, in either direction.**
//!
//! Note the three measures are *not* three independent votes — they are taken
//! from the same tallied spectrum in the same runs and are strongly correlated.
//! Their agreeing in sign is close to one observation, not three.
//!
//! An after-the-fact hypothesis, labelled as such because it was formed after
//! seeing the numbers and has not been tested: in a 55.8 %-leakage bare sphere,
//! raising the transport mean free path preferentially removes the *fast*
//! neutrons most likely to escape, which softens the surviving in-core flux.
//! That would oppose the per-collision hardening and is tied to the same
//! leakage that produced the reactivity effect. **The measurement that would
//! separate them is the same spectrum comparison run with a reflective
//! boundary** (`k_inf`, no leakage), where only the per-collision term
//! survives — the same decomposition `op-tm9f` used.
//!
//! # Results (2026-09-16, 128 seeds per arm, ENDF/B-VIII.0)
//!
//! | arm | n | mean vs ICSBEP | sd | sem |
//! |---|---|---|---|---|
//! | **ANISO** (evaluated MF=6 LANG=1) | 128 | **−26 pcm** | 197 | ±17 |
//! | **ISO** (pre-`op-og56`) | 128 | **+11 pcm** | 166 | ±15 |
//! | **difference** | | **−38 pcm** | | **±23 (1.6 sigma)** |
//!
//! **The prediction held on direction and on magnitude, and the result is still
//! a BOUND rather than a measurement.** At 1.6 sigma it is consistent with
//! zero. What it excludes is the alternative: an effect the size of `op-tm9f`'s
//! `−198 pcm` would sit **7 sigma** away. The continuum angular law is not a
//! second `op-tm9f`, which was the falsifiable half of the prediction.
//!
//! Do not quote `−38 pcm` as the worth of this law. Quote it as *"consistent
//! with zero; bounded below 70 pcm at 3 sigma; central value negative, as
//! predicted"*.
//!
//! **Superseding, not confirming, an earlier 32-seed run** which gave
//! `−41 ± 43 pcm`. That run used seeds 1…32 and this one seeds 1…128, so the
//! earlier sample is a **subset** of this one and the two are not independent.
//! The agreement is reassuring about stability but is not a second measurement,
//! and pooling them would double-count.
//!
//! **A harness check that passes.** The ISO arm reproduces the behaviour this
//! crate had before `op-og56`, whose independently pooled value is
//! `+16 ± 11 pcm` over 256 seeds (`examples/godiva_keff_ensemble.rs`). Measured
//! here at `+11 ± 15`, a difference of `−5 ± 19 pcm` — **0.3 sigma**. An
//! ablation arm landing back on a number pooled by a different program checks
//! the instrument rather than restating it.
//!
//! **A consequence for `RECORDED_PCM`.** That constant is `16.0`, measured
//! pre-`op-og56`. This says the post-`op-og56` mean is lower by `38 ± 23 pcm`,
//! i.e. near `−22`. It has **not** been re-measured at 256 seeds and the
//! constant is **not** changed on a 1.6 sigma shift. The drift gate that uses it
//! is `4·√(σ_run² + 11²) ≈ 693 pcm` for a single run, so a 38 pcm move is
//! nowhere near tripping it and nothing is currently mis-gated.
//!
//! **The seeds do not pair**, and the run says so: paired `sd` 246 exceeds
//! either arm's 197. Quote the unpaired figure. That is measured rather than
//! assumed — which matters here, because unlike the discrete-level ablation the
//! two arms *do* consume identical RNG variates per collision, so pairing might
//! have been expected to help. It does not: the histories diverge in where they
//! go, not in how many draws they take.
//!
//! **What would resolve it.** Getting `−38` to 3 sigma needs `sigma_diff ≈ 13`,
//! i.e. roughly **400 seeds per arm**. Nothing in this crate currently depends
//! on the number being resolved rather than bounded.
//!
//! # Results (2026-09-26, 400 seeds per arm, commit `3f141992e`)
//!
//! | arm | n | mean vs ICSBEP | sd | sem |
//! |---|---|---|---|---|
//! | **ANISO** | 400 | **−1 pcm** | 188 | ±9 |
//! | **ISO** | 400 | **−2 pcm** | 193 | ±10 |
//! | **difference** (unpaired) | | **+2 pcm** | | **±13 (0.1 sigma)** |
//!
//! Paired `sd` 259 again exceeds either arm's, so the unpaired figure is the
//! one to quote. **This supersedes the 128-seed `−38 ± 23` as the worth**, and
//! it does not confirm that run's negative central value: at the statistics the
//! old record said would resolve it, the law's worth is **consistent with zero
//! and bounded below 40 pcm at 3 sigma**. The "small" half of the prediction
//! held; the "negative" half is not supported.
//!
//! **Not a pooled extension of the 128-seed run.** The code moved between the
//! two (URR and DBRC default-on from 2026-09-20, the rdfil2-faithful PURR grid,
//! among others), so both arms shifted: ANISO `−26 → −1`, ISO `+11 → −2`. The
//! difference is the like-for-like quantity; the arm means are not comparable
//! across the dates. Also not re-measured on the RECONR/parser changes of
//! 2026-09-26 (commit `b039d06f3`), which move cross sections at
//! the 1e-16 level and the lumped MT=103-107 sections.
//!
//! ```text
//! OUTRAM_GODIVA_SEEDS=32 cargo run --release -p outram-mc-libs \
//!     --features endf-pebble-cases --example godiva_continuum_anisotropy_ablation
//! ```

#[cfg(target_os = "android")]
fn main() {
    println!("godiva_continuum_anisotropy_ablation is desktop-only (reads reference-data/endf/).");
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
    ///
    /// Taken from the machine, not hard-coded. This example exists to be moved
    /// to a bigger box, and a constant is one more thing a reader has to
    /// remember to raise -- the hand-off used to say exactly that. The result
    /// does not depend on it: seeds are chunked in order and each writes its
    /// own slot, so a run on 4 threads and a run on 64 produce the same
    /// per-seed vector, not merely the same mean.
    fn workers() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    }

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
        let chunk = seeds.len().div_ceil(workers().min(seeds.len()).max(1));
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

        // The control, checked before spending an hour of transport on two arms
        // that might be identical: at least one nuclide must actually carry a
        // continuum angular law, or the "ablation" removes nothing and the run
        // reports a null result that reads as "this physics does not matter".
        let carriers: Vec<&str> = aniso
            .iter()
            .filter(|n| n.has_continuum_anisotropy())
            .map(|n| n.name.as_str())
            .collect();
        assert!(
            !carriers.is_empty(),
            "not one of the three ICSBEP nuclides carries a continuum angular law, so this \
             ablation would remove nothing and report a meaningless null. Either the MF=6 \
             coefficients stopped being read (bead op-og56) or the tapes changed."
        );
        println!(
            "  continuum angular law present on: {}",
            carriers.join(", ")
        );

        let iso: Vec<Nuclide> = aniso
            .iter()
            .cloned()
            .map(Nuclide::with_isotropic_continuum_scattering)
            .collect();
        assert!(
            iso.iter().all(|n| !n.has_continuum_anisotropy()),
            "the ablation left a continuum angular law in place on at least one nuclide -- it is \
             a no-op, and any difference measured below would be an artefact"
        );
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

        // Paired: the arms share seeds. Unlike the discrete-level ablation, the
        // two arms here consume the SAME number of RNG draws per collision (one
        // variate, spent either inverting a cosine CDF or mapping it linearly),
        // so the streams do not diverge at the first continuum collision and the
        // pairing has a real chance of helping. Whether it does is measured, not
        // assumed — both figures are printed and the run says which to quote.
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
                "  -> paired sd ({sdp:.0}) EXCEEDS either arm's ({:.0}); the seeds do not pair, \
                 so quote the unpaired figure.",
                sa.max(sb)
            );
        } else {
            println!(
                "  -> paired sd ({sdp:.0}) is below either arm's ({:.0}); the pairing helps, \
                 so quote the paired figure.",
                sa.max(sb)
            );
        }

        println!(
            "\n  Prediction on op-og56, recorded before this ran: ANISO − ISO is SMALL and\n  \
             NEGATIVE — well under 50 pcm, plausibly under 20. U-238's MT=91 law is exactly\n  \
             isotropic below 1.2 MeV and only reaches <mu_cm> = +0.27 at 14 MeV, where a\n  \
             fission spectrum has little flux. A result near -200 pcm would mean the law is\n  \
             being applied at the wrong incident energy, not that it is worth that much."
        );
        let resolvable = 3.0 * ediff;
        if diff.abs() < resolvable {
            println!(
                "  NOTE: |{diff:+.0}| is inside 3 sigma ({resolvable:.0} pcm) of this run's own\n  \
                 statistics, so it is consistent with zero AND with the prediction. That is a\n  \
                 bound, not a measurement — raise OUTRAM_GODIVA_SEEDS to resolve it."
            );
        }
    }
}
