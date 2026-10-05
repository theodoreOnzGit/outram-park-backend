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
//! # Seed count: 32 is the standard (maintainer decision, 2026-09-18)
//!
//! **32 seeds is the bar for this workspace's pooled benchmark numbers. 256 is
//! overkill and is not required.** Decided after the 32-seed regeneration
//! below; do not spend the runtime re-litigating it.
//!
//! What that buys and what it costs, measured rather than asserted:
//!
//! | seeds | sem | runtime (this case) |
//! |---|---|---|
//! | 32 | +/-34 pcm | 289 s transport |
//! | 256 | +/-11 pcm | ~40 min transport |
//!
//! **Regeneration, 2026-09-18, 32 seeds:** `mean -55 pcm, sd 194, sem +/-34`,
//! against the recorded 256-seed `+16 +/- 11`. That is a **-71 pcm move at
//! 2.0 sigma** — the drift gate passes and both sit inside ICSBEP's own
//! +/-100 pcm band. Whether the -71 is ordinary fluctuation (2 sigma happens
//! ~5 % of the time at this seed count) or a small real drift from the MT=27
//! capture change and RECONR lump synthesis is **NOT resolved**, and at 32
//! seeds it cannot be. That is an accepted cost of the decision above, stated
//! here so it is not mistaken for a clean reproduction.
//!
//! **RESOLVED 2026-10-05 (GitHub #546), at 1024 seeds:** the current code sits
//! at **`−6 ± 5 pcm`**, see "Results (2026-10-05)" below. The two 32-seed
//! numbers that read low (`−55 ± 34` here on 2026-09-18, `−52 ± 27` on the
//! five-route study's route 4 on 2026-09-30) were 32-seed draws; the route-4
//! one was checked seed for seed (see below).
//!
//! **Consequence for quoting.** `+16 +/- 11` is a 256-seed figure and must not
//! be quoted off a 32-seed run, which yields +/-34. Against the benchmark's
//! own +/-100 pcm band `+16` and `-55` are the same answer, so report
//! agreement against the EXPERIMENT's band, never against our sem.
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
//! # Results (2026-10-05, 1024 seeds, ENDF/B-VIII.0) — the current record
//!
//! ```text
//!   seeds         1024
//!   mean          -6 pcm
//!   seed-to-seed  sd  165 pcm   (what ONE run scatters by)
//!   uncertainty   sem ±5 pcm    (on this pooled mean)
//!   distance from benchmark: 1.1 sem
//!   seed consistency: chi2/dof 0.97 (dof 1023, 95 % band [0.92, 1.09]),
//!     no outlier seed, 1/sqrt(N) check over 256 groups of 4 = 1.01 +/- 0.04
//! ```
//!
//! **`k = 0.99994 ± 0.00005`, i.e. `−6 ± 5 pcm` from HEU-MET-FAST-001**, at
//! commit `6faff1ed8`, measured 2026-10-05 for GitHub #546 with
//! `OUTRAM_GODIVA_SEEDS=1024`. Both gates passed (no regression from the
//! then-recorded `+16`: 1.8 σ of 12 pcm; benchmark `|−6| ≤ 100`). It is now
//! `RECORDED_PCM` (in `mod desktop`).
//!
//! - **Seed count.** From a timed pilot (4 seeds, 12.5 s of transport on two
//!   workers): 1024 seeds is ~50 min here, and `sem ≈ 5 pcm` already makes
//!   the comparison with the `−52 ± 27` it was run to check limited by that
//!   record's own σ. This exceeds the 32-seed standard deliberately: it
//!   re-points a drift gate, and that gate's `σ_recorded` should not be the
//!   larger term.
//! - **Hardware and time.** Intel Xeon Processor @ 2.10 GHz, **2 of 4 shared
//!   logical cores** (`taskset -c 2,3`; `default_workers()` = 2, each seed
//!   single-threaded), 15.7 GB RAM, Linux 6.18, CPU only, shared with another
//!   agent's runs. Data 114 s; transport 2995 s (2.9 s per seed per two
//!   workers).
//! - **Against `+16 ± 11`** (2026-09-15): −22 ± 12 pcm, 1.8 σ. Between the two,
//!   URR and DBRC became defaults and the #407 audit landed; their sum on
//!   Godiva is not decomposed here, and 1.8 σ is not claimed as a move.
//! - **Against route 4's `−52 ± 27`** (2026-09-30, 32 seeds): +46 ± 27 pcm,
//!   1.7 σ. Checked rather than accepted: `icsbep_five_route_keff` at
//!   `8b17079bc` reproduces route 4's per-seed `k` for seeds 1–32 **to every
//!   printed digit**, so the code did not move, and its seeds 33–288 give
//!   `−5 ± 12` (all 288: `−10 ± 11`, sd 183). That driver is multi-threaded
//!   and draws different streams per seed than this one, so the samples are
//!   independent; they agree to −4 ± 12 pcm. The −52 was a low draw.
//! - **Against the experiment:** inside its ±100 pcm band. As below, `−6`
//!   is not "6 pcm accuracy": the reference's own band is ±100.
//!
//! # ~~Results (2026-09-15, 256 seeds, ENDF/B-VIII.0)~~ — superseded 2026-10-05, kept as history
//!
//! ```text
//!   seeds         256
//!   mean          +16 pcm
//!   seed-to-seed  sd  173 pcm   (what ONE run scatters by)
//!   uncertainty   sem ±11 pcm   (on this pooled mean)
//!   distance from benchmark: 1.5 sem
//! ```
//!
//! **`+16 ± 11 pcm` from the ICSBEP benchmark**, inside its `±100 pcm`
//! experimental band, and `−198 pcm` from the `+214` recorded before the
//! discrete-inelastic angular distributions were sampled (bead `op-tm9f`). The
//! prediction on that bead, recorded *before* the work, was `−168 / −181 /
//! −219 pcm` priced three independent ways.
//!
//! Three things this does **not** say, worth stating because a number this
//! close invites over-reading:
//!
//! - **It is not more accurate than the experiment.** ICSBEP quotes
//!   `1.0000 ± 0.0010`. A `±11 pcm` statistical uncertainty on our side does
//!   not see past a `±100 pcm` band on the reference — anything inside that
//!   band is agreement, and `+16` is not meaningfully better than `+80` would
//!   be. The tight sem describes our *sampling*, not the comparison.
//! - **It is one benchmark.** A bare fast HEU metal sphere, one geometry, one
//!   temperature. It says nothing about thermal systems or about the crate's
//!   other cases.
//! - **It is not a clean bill for the physics underneath.** The cross-code
//!   study against OpenMC found a `~69 pcm` *spectral* residual that lives in
//!   `k_inf` and is untouched by this (bead `op-os8x`); our spectrum is still
//!   0.45 % harder than OpenMC's. Two offsetting errors can land on the right
//!   `k`, which is why the spectral residual is tracked separately rather than
//!   declared closed by this agreement.
//!
//! The V&V write-up is in
//! `verification_and_validation/openmc_godiva_cross_code/README.md`.
//!
//! ```text
//! OUTRAM_GODIVA_SEEDS=256 cargo run --release -p outram-mc-libs \
//!     --features endf-pebble-cases --example godiva_keff_ensemble
//! ```
//!
//! **Since 2026-10-03 (GitHub #494)** the seed loop is the library runner
//! [`outram_mc_libs::stats::ensemble`], with the same seeds, chunking and
//! pooling formula, so the numbers above are reproduced unchanged; the run
//! additionally prints a seed-consistency report (`χ²/dof` of the seeds
//! against each run's internal `σ`, outlier seeds, and a disjoint-group check
//! of the `1/√N` law). ~~That report has **NOT YET been measured** (testing
//! deferred by maintainer, 2026-10-03).~~ **First measured 2026-10-05** on the
//! 1024-seed run above: `χ²/dof = 0.97`, no outliers, `1/√N` ratio
//! `1.01 ± 0.04`, all consistent.
//!
//! `OUTRAM_SPEED=standard|fast|very-fast` picks the nuclides' `SpeedTier`.
//! Unset means `fast` (the default, exactly the same `k` as `standard`);
//! `very-fast` coarsens RECONR/BROADR to 1 %, an approximation whose
//! measured effect is in `docs/profiling/speed_tiers_2026_09_27.md`.

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
    use outram_mc_libs::stats::ensemble::{
        consecutive_seeds, default_workers, run_seeds_with_progress, EnsembleReport,
    };
    use std::sync::Arc;
    use std::time::Instant;

    const TEMP_K: f64 = 293.6;
    const RADIUS_CM: f64 = 8.7407;
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
            // OUTRAM_U238_ENDF7=1 swaps U-238 ALONE to ENDF/B-VII.0, every other
            // nuclide held at VIII.0. Remapped HERE rather than in NUCLIDES
            // because that is a `const` and `env::var` is not const-callable.
            //
            // Isolating one nuclide matters: the four pooled ICSBEP residuals
            // split by U-238 content (Godiva -55, Jemima -253, HST-009 -38,
            // LCT-008 +165 pcm over 32 seeds) and the two U-238-heavy cases
            // disagree in SIGN. A whole-library swap cannot separate U-238
            // from U-235; this can.
            let file = if name == "U238" && std::env::var("OUTRAM_U238_ENDF7").is_ok() {
                "n-092_U_238-ENDF7.0.endf"
            } else {
                file
            };
            let Some(p) = reference_endf(file) else {
                println!(
                    "  missing {file} — set OUTRAM_PARK_ENDF_DIR or fetch the tape; skipping."
                );
                return;
            };
            let nuc = Nuclide::from_endf_file_with_speed(&p, name, TEMP_K, outram_mc_libs::vv::bench_speed())
                .unwrap_or_else(|e| panic!("from_endf_file_with_speed({}): {e}", p.display()));
            // OUTRAM_FROZEN_NUBAR=1 freezes nu-bar(E) at thermal. Paired with
            // the same knob on Jemima: the WORTH RATIO between the two cases is
            // the measurement, not either number alone. The inelastic ablation
            // eliminated itself at a ratio of 1.53 against a residual ratio of
            // 4.6, so a candidate has to break that scaling to survive.
            let nuc = if std::env::var("OUTRAM_FROZEN_NUBAR").is_ok() {
                nuc.with_frozen_nubar(0.0253)
            } else {
                nuc
            };

            // OUTRAM_URR=1 adds unresolved-resonance self-shielding (PURR
            // probability tables). This is the BARE arm of the pair: Godiva is
            // 93.7 % U-235 with no reflector, so if the URR hypothesis is
            // right this should be worth far LESS here than on Jemima, whose
            // reflector is 99.3 % U-238. A shift here as large as Jemima's
            // refutes the hypothesis.
            let nuc = if std::env::var("OUTRAM_URR").is_ok() {
                let tape = njoy_outram_park_fork::endf::tape::Tape::read_file(&p).expect("tape");
                let mat = tape.materials()[0];
                nuc.with_urr_probability_tables(&tape, mat, TEMP_K, 20, 16, 2000)
                    .expect("PURR")
            } else {
                nuc
            };

            // OUTRAM_ISO_ELASTIC=1 ablates the MF=4/MT=2 elastic angular law.
            // Godiva is the BARE arm of this pair: it has no reflector, so if
            // reflector return is what Jemima's residual is made of, this knob
            // should be worth far less here than there.
            let nuc = if std::env::var("OUTRAM_ISO_ELASTIC").is_ok() {
                nuc.with_isotropic_elastic_scattering()
            } else {
                nuc
            };
            nuclides.push(nuc);
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

        // GitHub #494: the seed loop is now the library runner
        // (`outram_mc_libs::stats::ensemble`), which uses exactly the scheme
        // this example had inline — seeds `1..=N`, chunked in order across
        // `available_parallelism()` workers, each seed writing its own slot —
        // so the per-seed values, and every number printed below them, are
        // unchanged. The runner also returns each run's internal sigma, which
        // feeds the seed-consistency report printed after the gates' inputs.
        let seeds = consecutive_seeds(n_seeds);
        let nuclides = Arc::new(nuclides);

        println!("{n_seeds} seeds, 5000 histories × [40 inactive + 120 active]…");
        let t = Instant::now();
        let runs = run_seeds_with_progress(
            &seeds,
            default_workers(),
            |seed| {
                let settings = KeffSettings {
                    n_particles: 5000,
                    n_inactive: 40,
                    n_active: 120,
                    temperature_k: TEMP_K,
                    seed,
                    ..KeffSettings::default()
                };
                let r = run_keff(RADIUS_CM, &material, &nuclides, &settings);
                ((r.k_mean - 1.0) * 1.0e5, r.k_std * 1.0e5)
            },
            |n| {
                if n % 16 == 0 {
                    println!("  {n} seeds done ({:.0} s)", t.elapsed().as_secs_f64());
                }
            },
        );
        println!("  all seeds done in {:.1} s\n", t.elapsed().as_secs_f64());

        let report = EnsembleReport::from_runs(runs);
        let pcm = report.values();
        // `raffles::estimators::pooled`, which is the formula this example
        // computed inline before #494 (mean = sum/n; sd from the n-1 sample
        // variance; sem = sd/sqrt(n)), bit for bit.
        let (mean, sd, sem) = (report.mean, report.sd, report.sem);

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

        // Report-only (GitHub #494): do the seeds scatter as their own internal
        // sigma says they should? Printed before the gates so it is seen even
        // when a gate fires. Nothing here feeds the gates.
        println!("\n  Seed consistency (internal sigma vs seed-to-seed scatter, #494):");
        print!("{}", report.consistency_summary("pcm"));

        gate(mean, sem, sd, pcm.len());
    }

    /// The regression gates. Separated from [`run`] so the criteria are read as
    /// criteria rather than as print statements.
    ///
    /// # How these are sized, and why not at the target
    ///
    /// The tempting gate is `|mean| <= 30 pcm`, the accuracy this case now
    /// achieves. That gate would be **wrong**, and would fire on a correct
    /// build: at the default 32 seeds a run's own `sem` is ~31 pcm, so a true
    /// mean of `+16` produces observed means outside `±30` about a third of the
    /// time. A regression gate that cries wolf gets muted, and a muted gate
    /// guards nothing.
    ///
    /// So each gate is sized off **what the run in hand can resolve**:
    ///
    /// 1. **Resolving power.** Warn (do not fail) when `N` is too small for the
    ///    run to say anything useful. Failing here would punish a quick check.
    /// 2. **No regression from the recorded mean.** `|mean − RECORDED_PCM|`
    ///    within `4 sigma` of the *difference of two independent ensembles*,
    ///    `sigma_diff = sqrt(sem_run^2 + sem_recorded^2)` — not `4 × sem_run`,
    ///    which would treat the recorded value as exact and make the gate too
    ///    tight by `√2`. That error is on record in this crate
    ///    (`assert_reproduces_keff`, gh:#196), so it is spelled out here.
    /// 3. **Agreement with the experiment.** The claim being guarded is that
    ///    Godiva lands on the benchmark, so `|mean|` must be inside the
    ///    **experiment's own ±100 pcm band**, or within `4 sigma` of zero when
    ///    the run is too noisy for that to mean anything. Note this gate is
    ///    *weaker* than gate 2 by construction: it is the physics claim, while
    ///    gate 2 is what actually catches a code change.
    fn gate(mean: f64, sem: f64, sd: f64, n: usize) {
        /// `sem` on [`RECORDED_PCM`]: 165/sqrt(1024), 2026-10-05
        /// (~~173/sqrt(256) = 11~~ before).
        const RECORDED_SEM: f64 = 5.0;
        /// Sigma multiplier. 4 rather than 2 so a correct build essentially
        /// never trips it; a real regression here is hundreds of pcm, not tens.
        const K_SIGMA: f64 = 4.0;
        /// The ICSBEP experimental uncertainty on HEU-MET-FAST-001 \[pcm\].
        const BENCHMARK_BAND_PCM: f64 = 100.0;

        println!();
        if sem > 50.0 {
            println!(
                "  NOTE: {n} seeds give sem ±{sem:.0} pcm, too coarse to resolve this case's                  recorded {RECORDED_PCM:+.0} pcm offset. The gates below still run, but they are                  wide. Use OUTRAM_GODIVA_SEEDS=256 (sem ±10) or more for a number worth quoting."
            );
        }

        // 2. No regression from the recorded mean.
        let sigma_diff = (sem * sem + RECORDED_SEM * RECORDED_SEM).sqrt();
        let moved = (mean - RECORDED_PCM).abs();
        assert!(
            moved <= K_SIGMA * sigma_diff,
            "Godiva moved to {mean:+.0} pcm from the recorded {RECORDED_PCM:+.0} pcm --              {moved:.0} pcm, which is {:.1} sigma of the {sigma_diff:.0} pcm expected from              re-randomisation alone ({n} seeds, sem ±{sem:.0}; recorded sem ±{RECORDED_SEM:.0}).              Something changed the physics. If the change was deliberate, re-measure at 256              seeds and update RECORDED_PCM with the date and what moved it -- do NOT widen              this gate.",
            moved / sigma_diff
        );

        // 3. Agreement with the experiment.
        let benchmark_tol = BENCHMARK_BAND_PCM.max(K_SIGMA * sem);
        assert!(
            mean.abs() <= benchmark_tol,
            "Godiva sits {mean:+.0} pcm from ICSBEP HEU-MET-FAST-001, outside the tolerance of              {benchmark_tol:.0} pcm (the experiment's own ±{BENCHMARK_BAND_PCM:.0} pcm band, or              {K_SIGMA:.0} sigma of this run's ±{sem:.0} pcm, whichever is larger). This crate              agreed with the benchmark when the discrete inelastic angular distributions landed              (op-tm9f); it no longer does."
        );

        // A sanity check on the ensemble itself: a collapsed sd means the seeds
        // are not independent, which would make every uncertainty above a
        // fiction. See op-rbo, where exactly that defect once made the quoted
        // sigma meaningless while barely moving the central value.
        assert!(
            sd > 50.0,
            "seed-to-seed sd is {sd:.0} pcm, against ~{RECORDED_SD:.0} expected. That is too              small for {n} independent 5000-history runs: the per-particle RNG streams are              probably no longer independent, which would make every uncertainty reported here              meaningless -- including the agreement asserted above."
        );

        println!("  GATES PASSED:");
        println!(
            "    no regression: {mean:+.0} vs recorded {RECORDED_PCM:+.0} pcm              ({:.1} sigma of {sigma_diff:.0})",
            moved / sigma_diff
        );
        println!("    benchmark:     |{mean:+.0}| <= {benchmark_tol:.0} pcm");
        println!("    seeds independent: sd {sd:.0} pcm");
    }

    /// The pooled Godiva offset from ICSBEP HEU-MET-FAST-001, in pcm, as last
    /// measured by this example: **`−6 pcm`, 1024 seeds, `sem ±5`,
    /// seed-to-seed `sd 165`, 2026-10-05, commit `6faff1ed8`** (GitHub #546;
    /// method, hardware and the checks against `+16` and route 4 in this
    /// file's "Results (2026-10-05)"). ~~`+16 pcm`, 256 seeds, `sem ±11`,
    /// seed-to-seed `sd 173`, 2026-09-15~~.
    ///
    /// History, so a reader can see which numbers were resolvable and which
    /// were single draws read as answers:
    ///
    /// | value | basis | what changed |
    /// |---|---|---|
    /// | `−341` | 1 seed | first two-nuclide Godiva |
    /// | `+57 ± 173` | 1 seed | three nuclides; unresolvable |
    /// | `+314 ± 21` | 96 seeds | after the MT=91 Q-value cap (gh:#192) |
    /// | `+214 ± 20` | 64 seeds | after reading the evaluated MF=6 continuum law |
    /// | ~~`+16 ± 11`~~ | 256 seeds | after sampling the MF=4 discrete inelastic angular laws (`op-tm9f`); superseded 2026-10-05 |
    /// | **`−6 ± 5`** | **1024 seeds** | **after URR + DBRC became defaults and the #407 audit (−22 ± 12 vs the row above, 1.8 σ, not decomposed)** |
    ///
    /// Note the two single-draw rows: both were superseded by pooled runs that
    /// moved them by more than their own quoted uncertainty. That is why this
    /// example exists and why the gate below is sized off `sem`, not off one
    /// run.
    pub const RECORDED_PCM: f64 = -6.0;

    /// Seed-to-seed standard deviation of a single run \[pcm\], measured at
    /// 1024 seeds on 2026-10-05 (~~173 at 256 seeds~~). Quoted so a reader can
    /// size their own run: `sem = SD / √N`.
    pub const RECORDED_SD: f64 = 165.0;
}
