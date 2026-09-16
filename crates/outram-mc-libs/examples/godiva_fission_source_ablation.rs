//! **Price the two factors of the fission source on Godiva** — paired-seed
//! ablations of ν̄(E) and χ(E→E'), the hooks that landed with `5b6df9b`
//! (coverage gap 4 of `docs/neutronics-physics-coverage.md`).
//!
//! Job 5 of `docs/handoff-heavy-neutronics-runs.md`. Both hooks were verified
//! against the tape by `tests/ablation_hook_controls.rs` — that a switch
//! switches — but neither had ever been run on a case, so neither factor of
//! `ν̄ χ` had a *price*. This is that measurement.
//!
//! # What is being ablated
//!
//! Three arms over the **same seeds**, identical in every other respect —
//! geometry, the three ICSBEP nuclide densities, temperature, histories,
//! generation split, and every cross section.
//!
//! - **BASE** — the evaluation as it stands: ENDF MF=1/MT=452 ν̄(E) and the
//!   MF=5/MT=18 LF=1 χ(E→E').
//! - **NU-FROZEN** — [`Nuclide::with_frozen_nubar`] at `E_ref`. ν̄ is read at
//!   `E_ref` whatever the incident energy, so its *slope* is removed and its
//!   magnitude at `E_ref` is kept.
//! - **CHI-FROZEN** — [`Nuclide::with_frozen_fission_spectrum`] at `E_ref`.
//!   Every fission emits as though induced at `E_ref`.
//!
//! They *freeze* rather than remove because a zero ν̄ is not an ablation, it is
//! a subcritical block of metal.
//!
//! `E_ref` defaults to **thermal, 0.0253 eV** (`OUTRAM_GODIVA_FREEZE_EV` to
//! override), which answers "what if the curve had never risen". Freezing
//! instead at the flux-average incident energy answers the narrower and
//! better-conditioned "what does the *shape* cost at fixed mean yield"; run it
//! as a second pair if there is budget.
//!
//! # Predictions, recorded in the hand-off before this program existed
//!
//! From the control tests' own numbers on ENDF/B-VIII.0 U-235:
//!
//! | arm | the data | predicted worth on Godiva |
//! |---|---|---|
//! | NU-FROZEN | ν̄ 2.42985 @ 0.0253 eV vs 2.64574 @ 2 MeV; `nu_fission` falls **−8.16 %** at 2 MeV when frozen at thermal | **large and negative — thousands of pcm** |
//! | CHI-FROZEN | χ's tape rows integrate to a mean birth energy **+0.883 %** across thermal → 14 MeV | **very small — well under 100 pcm** |
//!
//! The two differ by roughly two orders of magnitude, so this is a test of
//! whether the hooks are *wired*, not only a measurement. A large CHI-FROZEN
//! reading means the wiring, not the physics.
//!
//! # Statistics
//!
//! The ν̄ hook **preserves the RNG stream** (ν̄ is read, not sampled), so a
//! paired difference is attributable history by history. The χ hook does
//! **not** in general — MF=5 laws are rejection-sampled and a birth draw's
//! variate count can depend on the incident energy — so its difference is an
//! ensemble statement only. Both the paired and unpaired figures are printed
//! for every arm and the program says which to quote, measured rather than
//! assumed; that is the same discipline as
//! `examples/godiva_continuum_anisotropy_ablation.rs`, whose statistics were
//! got wrong twice before they were got right (gh:#196).
//!
//! ```text
//! OUTRAM_GODIVA_SEEDS=64 cargo run --release -p outram-mc-libs \
//!     --features endf-pebble-cases --example godiva_fission_source_ablation
//! ```

#[cfg(target_os = "android")]
fn main() {
    println!("godiva_fission_source_ablation is desktop-only (reads reference-data/endf/).");
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
    /// Default freeze energy \[eV\]: thermal, 2200 m/s.
    const DEFAULT_FREEZE_EV: f64 = 0.0253;
    /// A probe inside Godiva's flux, well above every threshold of interest.
    const PROBE_EV: f64 = 2.0e6;

    /// `(tape file, nuclide name, atom density \[atoms/barn·cm\])` — the three
    /// ICSBEP nuclides, same numbers as `godiva_keff_endf_local`.
    const NUCLIDES: &[(&str, &str, f64)] = &[
        ("n-092_U_234-ENDF8.0.endf", "U234", 4.9184e-4),
        ("n-092_U_235-ENDF8.0.endf", "U235", 4.4994e-2),
        ("n-092_U_238.endf", "U238", 2.4984e-3),
    ];

    /// Worker threads — one whole seed each; transport inside a seed is serial.
    ///
    /// Every earlier ablation example hard-codes `WORKERS = 4` and the hand-off
    /// tells the next person to edit it. Reading the machine instead means one
    /// fewer edit to forget, and `OUTRAM_GODIVA_WORKERS` still overrides it.
    fn workers() -> usize {
        std::env::var("OUTRAM_GODIVA_WORKERS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&w| w > 0)
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4)
            })
    }

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
        let chunk = seeds.len().div_ceil(workers());
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

    /// Mean birth energy \[eV\] over `n` draws at incident energy `e_in`.
    fn mean_birth(nuc: &Nuclide, e_in: f64, n: usize, seed: u64) -> f64 {
        let mut s = seed;
        (0..n)
            .map(|_| nuc.sample_fission_energy(e_in, &mut s))
            .sum::<f64>()
            / n as f64
    }

    /// Report one ablated arm against BASE, unpaired and paired.
    ///
    /// Returns the figure the run says to quote and its 1σ error, so the
    /// verdict below reads the same number a human would.
    fn report(label: &str, base: &[f64], arm: &[f64]) -> (f64, f64) {
        let (mb, sb, eb) = stats(base);
        let (ma, sa, ea) = stats(arm);
        let diff = ma - mb;
        let ediff = (ea * ea + eb * eb).sqrt();
        let d: Vec<f64> = arm.iter().zip(base).map(|(x, y)| x - y).collect();
        let (md, sdp, edp) = stats(&d);

        println!("  {label} − BASE");
        println!(
            "    unpaired = {:+.0} ± {:.0} pcm  ({:.1} sigma)",
            diff,
            ediff,
            (diff / ediff).abs()
        );
        println!(
            "    paired   = {:+.0} ± {:.0} pcm  ({:.1} sigma), paired sd {:.0}",
            md,
            edp,
            (md / edp).abs(),
            sdp
        );
        if sdp > sa.max(sb) {
            println!(
                "    -> paired sd ({sdp:.0}) EXCEEDS either arm's ({:.0}); the seeds do not \
                 pair, so quote the UNPAIRED figure.",
                sa.max(sb)
            );
            (diff, ediff)
        } else {
            println!(
                "    -> paired sd ({sdp:.0}) is below either arm's ({:.0}); the pairing helps, \
                 so quote the PAIRED figure.",
                sa.max(sb)
            );
            (md, edp)
        }
    }

    pub fn run() {
        let n_seeds: usize = std::env::var("OUTRAM_GODIVA_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(16);
        let e_ref: f64 = std::env::var("OUTRAM_GODIVA_FREEZE_EV")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_FREEZE_EV);

        println!("Reconstructing HEU isotopes from ENDF/B-VIII.0 (RECONR + BROADR @ {TEMP_K} K)…");
        let t0 = Instant::now();
        let mut base = Vec::new();
        for &(file, name, _) in NUCLIDES {
            let Some(p) = reference_endf(file) else {
                println!("  missing {file} — set OUTRAM_PARK_ENDF_DIR or fetch the tape; skipping.");
                return;
            };
            base.push(
                Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
                    .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display())),
            );
        }

        // ── Controls, before spending hours of transport on three arms ──
        //
        // The first is the one usually skipped and the one that matters most: a
        // hook that removes nothing reports "no difference", and that reads as
        // "this physics does not matter". `tests/ablation_hook_controls.rs`
        // asserts all of this at the nuclide level; it is repeated here against
        // THESE THREE nuclides at THIS temperature, because a control that
        // passed on a different tape does not license this run.

        // 1a: nu-bar genuinely varies with energy on at least one fissile
        //     nuclide, so there is a slope to freeze out.
        let nu_movers: Vec<String> = base
            .iter()
            .filter(|n| {
                let lo = n.nu_bar(e_ref);
                let hi = n.nu_bar(PROBE_EV);
                lo > 0.0 && (hi - lo).abs() > 0.0
            })
            .map(|n| {
                format!(
                    "{} ({:.5} @ {:.4e} eV -> {:.5} @ {:.1e} eV, {:+.3} %)",
                    n.name,
                    n.nu_bar(e_ref),
                    e_ref,
                    n.nu_bar(PROBE_EV),
                    PROBE_EV,
                    100.0 * (n.nu_bar(PROBE_EV) / n.nu_bar(e_ref) - 1.0)
                )
            })
            .collect();
        assert!(
            !nu_movers.is_empty(),
            "not one of the three ICSBEP nuclides has a nu-bar that varies between {e_ref} eV \
             and {PROBE_EV} eV, so freezing it removes nothing and this run would report a \
             meaningless null."
        );
        println!("  nu-bar slope present on:");
        for m in &nu_movers {
            println!("    {m}");
        }

        // 1b: chi genuinely depends on the incident energy. Measured through
        //     the sampler this run actually uses, not through the tape, so a
        //     law that parses but is never sampled cannot pass.
        const N_DRAWS: usize = 400_000;
        let chi_movers: Vec<String> = base
            .iter()
            .filter(|n| n.nu_bar(PROBE_EV) > 0.0)
            .filter_map(|n| {
                let lo = mean_birth(n, e_ref, N_DRAWS, 0xF1_5510_0001);
                let hi = mean_birth(n, PROBE_EV, N_DRAWS, 0xF1_5510_0001);
                (lo > 0.0 && (hi - lo).abs() > 0.0).then(|| {
                    format!(
                        "{} (mean birth {lo:.5e} eV @ {:.4e} -> {hi:.5e} eV @ {:.1e}, {:+.3} %)",
                        n.name,
                        e_ref,
                        PROBE_EV,
                        100.0 * (hi / lo - 1.0)
                    )
                })
            })
            .collect();
        assert!(
            !chi_movers.is_empty(),
            "no fissile nuclide's sampled fission spectrum moved between {e_ref} eV and \
             {PROBE_EV} eV incident. A static Watt stand-in has no incident-energy dependence \
             to freeze, so the chi arm would price nothing while reporting a number."
        );
        println!("  chi incident-energy dependence present on:");
        for m in &chi_movers {
            println!("    {m}");
        }

        // 1c: WHERE chi's incident-energy dependence lives, scanned through the
        //     sampler this run uses.
        //
        //     This is a diagnostic, not a gate, and it is here because the
        //     prediction it informs is stated as a bound: the hand-off predicts
        //     "well under 100 pcm" from the tape's +0.883 % in mean birth energy
        //     between the 1e-5 eV and 14 MeV ROWS. That is an endpoint
        //     statement, and a fission spectrum is not born at either endpoint —
        //     it is born around 1-3 MeV. If the sampled curve bulges between the
        //     rows the prediction was calibrated on the wrong quantity, and this
        //     table is what says so before the transport arms are read.
        println!("  chi mean birth energy vs incident energy (sampler, {N_DRAWS} draws):");
        print!("    {:<6}", "E_in");
        for n in base.iter().filter(|n| n.nu_bar(PROBE_EV) > 0.0) {
            print!("  {:>22}", n.name);
        }
        println!();
        for &e_in in &[
            1.0e-5, 2.53e-2, 1.0e0, 1.0e3, 1.0e5, 5.0e5, 1.0e6, 2.0e6, 4.0e6,
            8.0e6, 1.4e7, 3.0e7,
        ] {
            print!("    {e_in:<6.1e}");
            for n in base.iter().filter(|n| n.nu_bar(PROBE_EV) > 0.0) {
                let m = mean_birth(n, e_in, N_DRAWS, 0xF1_5510_0003);
                let base_m = mean_birth(n, 1.0e-5, N_DRAWS, 0xF1_5510_0003);
                print!("  {m:>12.5e} ({:+6.2} %)", 100.0 * (m / base_m - 1.0));
            }
            println!();
        }

        let nu_frozen: Vec<Nuclide> = base
            .iter()
            .cloned()
            .map(|n| n.with_frozen_nubar(e_ref))
            .collect();
        let chi_frozen: Vec<Nuclide> = base
            .iter()
            .cloned()
            .map(|n| n.with_frozen_fission_spectrum(e_ref))
            .collect();

        // 2: the hooks took effect, and BASE is genuinely unablated.
        assert!(
            base.iter().all(|n| n.frozen_nubar_energy().is_none()
                && n.frozen_fission_spectrum_energy().is_none()),
            "the BASE arm is already ablated; its difference from the others would be an \
             artefact"
        );
        assert!(
            nu_frozen
                .iter()
                .all(|n| n.frozen_nubar_energy() == Some(e_ref)),
            "with_frozen_nubar did not take on every nuclide -- the ablation is a partial no-op"
        );
        assert!(
            chi_frozen
                .iter()
                .all(|n| n.frozen_fission_spectrum_energy() == Some(e_ref)),
            "with_frozen_fission_spectrum did not take on every nuclide"
        );

        // 3: the two factors stay separable, and each arm moves only its own.
        //    Freezing nu-bar must not touch a birth energy; freezing chi must
        //    not touch nu-bar. If they leak into one another the two numbers
        //    below are not the two mechanisms' worths.
        for (b, nuf, chif) in itertools_zip3(&base, &nu_frozen, &chi_frozen) {
            assert_eq!(
                b.nu_bar(PROBE_EV).to_bits(),
                chif.nu_bar(PROBE_EV).to_bits(),
                "{}: the chi ablation moved nu-bar",
                b.name
            );
            assert_eq!(
                mean_birth(b, PROBE_EV, 4096, 0xF1_5510_0002).to_bits(),
                mean_birth(nuf, PROBE_EV, 4096, 0xF1_5510_0002).to_bits(),
                "{}: the nu-bar ablation moved the fission spectrum",
                b.name
            );
            if b.nu_bar(PROBE_EV) > 0.0 {
                assert_ne!(
                    b.nu_bar(PROBE_EV).to_bits(),
                    nuf.nu_bar(PROBE_EV).to_bits(),
                    "{}: with_frozen_nubar left nu-bar unchanged at {PROBE_EV:.1e} eV",
                    b.name
                );
            }
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
        let base = Arc::new(base);
        let nu_frozen = Arc::new(nu_frozen);
        let chi_frozen = Arc::new(chi_frozen);

        println!(
            "{n_seeds} seeds per arm on {} workers, 5000 histories × [40 inactive + 120 \
             active], freeze energy {e_ref:.4e} eV…",
            workers()
        );
        let t = Instant::now();
        let b = run_arm(&base, &material, &seeds);
        println!("  BASE       arm done in {:.1} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let nu = run_arm(&nu_frozen, &material, &seeds);
        println!("  NU-FROZEN  arm done in {:.1} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let chi = run_arm(&chi_frozen, &material, &seeds);
        println!(
            "  CHI-FROZEN arm done in {:.1} s\n",
            t.elapsed().as_secs_f64()
        );

        println!("Δk from ICSBEP HEU-MET-FAST-001 = 1.0000, pcm");
        println!("  arm          n     mean      sd     sem");
        for (label, x) in [("BASE", &b), ("NU-FROZEN", &nu), ("CHI-FROZEN", &chi)] {
            let (m, s, e) = stats(x);
            println!("  {label:<10} {:>3}   {:+7.0}  {:>6.0}  {:>6.0}", x.len(), m, s, e);
        }
        println!();

        let (nu_q, nu_e) = report("NU-FROZEN", &b, &nu);
        println!();
        let (chi_q, chi_e) = report("CHI-FROZEN", &b, &chi);

        println!();
        println!("  Predictions recorded in docs/handoff-heavy-neutronics-runs.md before this");
        println!("  program was written:");
        println!("    NU-FROZEN  : LARGE and NEGATIVE -- thousands of pcm. nu_fission falls");
        println!("                 8.16 % at 2 MeV when nu-bar is frozen at thermal, and that");
        println!("                 is a direct multiplier on the fission source.");
        println!("    CHI-FROZEN : VERY SMALL -- well under 100 pcm. chi's own tape rows move");
        println!("                 only +0.883 % in mean birth energy from thermal to 14 MeV.");
        println!("  They differ by ~2 orders of magnitude, so this is a test of the WIRING as");
        println!("  much as a measurement.");

        // The verdicts read the same figure the run just told a human to quote.
        println!("\n  VERDICT, nu-bar slope:");
        if nu_q < 0.0 && nu_q.abs() > 1000.0 && nu_q.abs() > 3.0 * nu_e {
            println!(
                "    HELD. {nu_q:+.0} ± {nu_e:.0} pcm is negative, in the thousands, and \
                 resolved at {:.1} sigma.",
                (nu_q / nu_e).abs()
            );
        } else if nu_q.abs() < 3.0 * nu_e {
            println!(
                "    UNRESOLVED at 3 sigma: {nu_q:+.0} ± {nu_e:.0} pcm. Raise \
                 OUTRAM_GODIVA_SEEDS.\n    A nu-bar arm this quiet is a WIRING suspicion, not \
                 a small effect -- the hook is a\n    documented partial no-op on the LOW \
                 (Core) tier above the WMP e_max; check the tier."
            );
        } else {
            println!(
                "    FAILED as stated: {nu_q:+.0} ± {nu_e:.0} pcm ({:.1} sigma). Correct the \
                 prediction\n    where it is written down rather than reinterpreting it.",
                (nu_q / nu_e).abs()
            );
        }

        println!("  VERDICT, chi incident-energy dependence:");
        if chi_q.abs() + 3.0 * chi_e < 100.0 {
            println!(
                "    HELD. {chi_q:+.0} ± {chi_e:.0} pcm is bounded below 100 pcm at 3 sigma."
            );
        } else if chi_q.abs() < 3.0 * chi_e {
            println!(
                "    CONSISTENT WITH ZERO but the bound is loose: {chi_q:+.0} ± {chi_e:.0} pcm, \
                  3 sigma =\n    {:.0} pcm, which does not yet fit inside the predicted 100 \
                 pcm. Raise OUTRAM_GODIVA_SEEDS\n    before claiming the prediction held.",
                3.0 * chi_e
            );
        } else {
            println!(
                "    FAILED as stated: {chi_q:+.0} ± {chi_e:.0} pcm ({:.1} sigma) is outside \
                 the predicted\n    100 pcm. The hand-off says a larger chi reading means the \
                 WIRING, not the physics --\n    suspect that first, then correct the \
                 prediction where it is written down.",
                (chi_q / chi_e).abs()
            );
        }
        println!(
            "\n  Freeze energy was {e_ref:.4e} eV. Frozen at THERMAL this answers \"what if the\n  \
             curve had never risen\" — it moves magnitude and shape together. Freezing at the\n  \
             flux-average incident energy instead (OUTRAM_GODIVA_FREEZE_EV) answers the\n  \
             narrower \"what does the SHAPE cost at fixed mean yield\"."
        );
    }

    /// Three-way zip, so the separability control can walk the arms together
    /// without pulling in a dependency for it.
    fn itertools_zip3<'a>(
        a: &'a [Nuclide],
        b: &'a [Nuclide],
        c: &'a [Nuclide],
    ) -> impl Iterator<Item = (&'a Nuclide, &'a Nuclide, &'a Nuclide)> {
        a.iter().zip(b.iter()).zip(c.iter()).map(|((x, y), z)| (x, y, z))
    }
}
