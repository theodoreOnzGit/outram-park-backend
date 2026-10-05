//! Godiva bare-sphere criticality on the **HIGH tier**, from the repo's own
//! ENDF/B-VIII.0 tapes — no network.
//!
//! # Why this exists
//!
//! The FHR pebble study (`verification_and_validation/ring_rpt/`) sits ~+1.7 %
//! above its OpenMC reference and every *data* mechanism has been excluded by
//! measurement: point σ vs NJOY's PENDF (±0.04 %), the capture **resonance
//! integral** vs NJOY (+0.00 %) ~~and vs the published RI_∞~~ (that
//! reference has no recorded source, 2026-10-04, #524;
//! `u238_resonance_integral.rs`), graphite σ vs THERMR (±0.05 %), and the
//! slowing-down kernel above the S(α,β) cutoff against analytic two-body
//! kinematics (`epithermal_slowing_down.rs`, ξ/ξ₀ = 1.000).
//!
//! What has never been done is to point the **same HIGH data path and the same
//! power-iteration driver** at a problem with an *external, measured* answer.
//! Every comparison so far has been against another code. Godiva is ICSBEP
//! **HEU-MET-FAST-001**: a bare HEU metal sphere, r = 8.7407 cm, benchmark
//! `k_eff = 1.0000 ± 0.0010`. That is an experiment, not a code.
//!
//! It also splits the search cleanly, because Godiva is a *fast* system:
//!
//! | if HIGH-tier Godiva … | then the pebble offset is … |
//! |---|---|
//! | lands on 1.0000 | specific to thermal / epithermal physics, not the shared path |
//! | sits ~+1–2 % high too | a **general** bias in the HIGH path (ν̄, χ, fast σ, or the driver) |
//!
//! Godiva exercises ν̄(E), the MF=5 fission spectrum, U-235 fast fission,
//! inelastic levels and (n,2n) — and **no** thermal scattering and **no**
//! resonance escape. It is the complement of the pebble.
//!
//! `godiva_keff_endf` already does this, but only with `--features net-fetch`
//! against ENDF/B-VII.1 from the IAEA, which this environment cannot reach.
//! This twin reads `reference-data/endf/` instead and uses **ENDF/B-VIII.0** —
//! the same library and the same tapes the pebble study runs on, which is what
//! makes the split above valid.
//!
//! # This example carries the crate's maturity evidence (from 2026-09-12)
//!
//! `crates/outram-mc-libs/CLAUDE.md` declares this crate mature on *k* within
//! **500 pcm** of HEU-MET-FAST-001, reconstructed from an ENDF evaluation. On
//! 2026-09-12 this run measured **+57 ± 173 pcm** on all three ICSBEP nuclides,
//! putting the bar 2.9 sigma away — so the gate tests the bar and not the noise.
//!
//! **That +57 is now known to be a single-seed draw, not the code's answer**
//! (2026-09-13, gh:#196). A 96-seed paired study at these same settings measured
//! the then-current code's true mean at **+228 ± 18 pcm** (seed-to-seed
//! sd 178 pcm), which makes +57 a −0.97 sigma draw. The MT=91 Q-value cap of
//! gh:#192 then moved the case to a true mean of **+314 ± 21 pcm** (sd 205 pcm),
//! a real but small **+85 ± 26 pcm (3.2 sigma)** shift; the rest of the +363 pcm
//! seen between two single runs was re-randomisation.
//!
//! Reading the **evaluated MF=6 LAW=1 continuum law** for MT=91/MT=16, in place
//! of the Weisskopf evaporation stand-in, then moved it again — **−105 ± 32 pcm
//! (3.3 sigma)**, this time *toward* the benchmark, leaving
//! **+214 ± 20 pcm** (64 seeds per arm,
//! `examples/godiva_mf6_continuum_ensemble.rs`).
//!
//! Wiring in the **discrete inelastic MF=4 angular distributions** (`op-tm9f`,
//! 2026-09-15) moved it a further **−198 pcm**, to **+16 ± 11 pcm over 256
//! seeds** — inside the ICSBEP ±100 pcm band for the first time in this case's
//! history. ~~That is the value [`RECORDED_PCM`] now holds~~ **Superseded
//! 2026-10-05 (GitHub #546):** [`RECORDED_PCM`] now holds **−6 ± 5 pcm over 1024
//! seeds**, re-measured at `6faff1ed8` after the URR/DBRC defaults and the
//! OpenMC-parity audit; see its docs. `examples/godiva_keff_ensemble.rs` is
//! where it is measured and gated.
//!
//! The 500 pcm bar holds with 44 sigma of margin on the pooled number. **This
//! program cannot establish any of that**: it takes one draw, with a sigma of
//! ~173 pcm, so it is checked *against* the pooled value at the resolution one
//! draw has — see [`RECORDED_PCM`] for how that gate is sized (gh:#196).
//!
//! It took that role over from `examples/endf_to_keff.rs`, which could not
//! support it: sigma ~ 330 pcm there makes 500 pcm a 1.5 sigma envelope, so a
//! gate at the bar fires on noise and a gate at bar + 4 sigma is 1814 pcm —
//! 3.6x looser than the bar it claims to enforce. Neither tests 500 pcm. That
//! example also models two of Godiva's three nuclides (U-234 is absent). Its
//! `TUTORIAL_BAND_K` carries the full account.
//!
//! Note the gate below is **tighter than the bar**: it uses the ICSBEP band
//! itself (0.0010) plus 4 sigma, not the 500 pcm crate-level figure. 500 pcm is
//! the claim the crate makes overall; there is no reason to judge this
//! particular case loosely when it can afford not to be.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example godiva_keff_endf_local
//! ```
//!
//! `OUTRAM_SPEED=standard|fast|very-fast` picks the nuclides' `SpeedTier`.
//! Unset means `fast` (the default, exactly the same `k` as `standard`);
//! `very-fast` coarsens RECONR/BROADR to 1 %, an approximation whose
//! measured effect is in `docs/profiling/speed_tiers_2026_09_27.md`.

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
use outram_mc_libs::vv::{assert_reproduces_keff, godiva, RecordedKeff};
use std::time::Instant;

/// Godiva material temperature \[K\] (room temperature; the benchmark is a metal
/// assembly, not a reactor). Since 2026-10-04 the model's numbers live in
/// [`outram_mc_libs::vv::godiva`], shared with the tutorial and the web demo.
const TEMP_K: f64 = godiva::TEMPERATURE_K;

fn main() {
    println!("Reconstructing HEU isotopes from ENDF/B-VIII.0 (RECONR + BROADR @ {TEMP_K} K)…");
    let t0 = Instant::now();
    let nuclides: Vec<Nuclide> = godiva::NUCLIDES
    .iter()
    .map(|(name, file, _)| {
        let p = reference_endf(file).unwrap_or_else(|| panic!("missing reference tape {file}"));
        let t = Instant::now();
        let n = Nuclide::from_endf_file_with_speed(&p, name, TEMP_K, outram_mc_libs::vv::bench_speed())
            .unwrap_or_else(|e| panic!("from_endf_file_with_speed({}): {e}", p.display()));
        println!(
            "  {name}: reconstructed in {:.1} s",
            t.elapsed().as_secs_f64()
        );
        n
    })
    .collect();
    println!(
        "Nuclear data ready in {:.1} s.\n",
        t0.elapsed().as_secs_f64()
    );

    // HEU-MET-FAST-001 atom densities [atoms/barn·cm] — the same numbers the
    // LOW-tier `godiva_keff` and the net-fetch `godiva_keff_endf` use, so the
    // three runs differ only in where σ comes from. (4.9184e-4, 4.4994e-2,
    // 2.4984e-3 for U-234/235/238, in `vv::godiva::NUCLIDES`.)
    let material = godiva::material();

    // Defaults 5000 x [40 + 120]; OUTRAM_NPART / OUTRAM_NINACTIVE /
    // OUTRAM_NACTIVE override them for profiling (`vv::bench_run_size`).
    let (n_particles, n_inactive, n_active) = outram_mc_libs::vv::bench_run_size(5000, 40, 120);
    let settings = KeffSettings {
        n_particles,
        n_inactive,
        n_active,
        temperature_k: TEMP_K,
        ..KeffSettings::default()
    };

    // Try changing this: the lesson's "find the critical radius" exercise.
    let radius_cm = godiva::RADIUS_CM;
    println!("Godiva bare-sphere Keff — HIGH fidelity, ENDF/B-VIII.0  (r = {radius_cm} cm)");
    eprintln!("  speed tier: {} (OUTRAM_SPEED)", outram_mc_libs::vv::bench_speed());
    println!(
        "  {} histories/gen, {} inactive + {} active generations\n",
        settings.n_particles, settings.n_inactive, settings.n_active
    );

    let t_mc = Instant::now();
    let result = run_keff(radius_cm, &material, &nuclides, &settings);
    println!("  transport: {:.1} s", t_mc.elapsed().as_secs_f64());
    println!("  k_eff = {:.5} ± {:.5}", result.k_mean, result.k_std);
    println!("  ICSBEP HEU-MET-FAST-001 = 1.0000 ± 0.0010");
    let pcm = (result.k_mean - 1.0) * 1.0e5;
    let sigma = result.k_std * 1.0e5;
    println!("  Δk from benchmark = {pcm:+.0} ± {sigma:.0} pcm");

    // ── V&V gate ──────────────────────────────────────────────────────────────
    //
    // The benchmark's own band is 0.0010 in k; the gate adds 4 sigma of this
    // run's statistics on top. Four, not one: a V&V gate that fires on ordinary
    // statistical fluctuation trains people to ignore it.
    //
    // The recorded result is ~~**+214 ± 20 pcm**, the pooled mean of a 64-seed
    // ensemble of this program at these settings (2026-09-13)~~ (stale since
    // 2026-09-15; CORRECTED 2026-10-05) **−6 ± 5 pcm**, the pooled mean of
    // 1024 seeds of this program's model at these settings (2026-10-05, #546;
    // see `RECORDED_PCM`); the single-seed +57 of 2026-09-12 was a −0.97 sigma
    // draw of an earlier code. It is
    // checked as a SEPARATE claim from agreement with the benchmark: a result
    // can stay inside the ICSBEP band while drifting steadily within it, and
    // only the reproduction claim sees that.
    //
    // The drift gate is 4 sigma of THIS run's statistics, so it scales with
    // however many histories the run was given.
    println!("\n=== V&V gate: ICSBEP HEU-MET-FAST-001 ===");
    assert_reproduces_keff(
        "HEU-MET-FAST-001 (Godiva), HIGH tier, ENDF/B-VIII.0",
        result.k_mean,
        result.k_std,
        ICSBEP_HMF001_K,
        ICSBEP_HMF001_BAND,
        Some(RecordedKeff::pooled(RECORDED_PCM, RECORDED_SEM_PCM)),
    );
}

/// ICSBEP **HEU-MET-FAST-001** (Godiva) benchmark `k_eff`.
///
/// A bare HEU metal sphere, r = 8.7407 cm. The configuration is critical by
/// construction, so the benchmark value is exactly 1.0000; the band is the
/// evaluation's own stated uncertainty. This is an **experiment**, not another
/// code's answer — which is the whole reason this case is here, since every
/// other comparison in this crate's V&V set is against NJOY, OpenMC, or an
/// analytic limit.
const ICSBEP_HMF001_K: f64 = godiva::BENCHMARK_K;

/// The ICSBEP-stated uncertainty on [`ICSBEP_HMF001_K`].
const ICSBEP_HMF001_BAND: f64 = godiva::BENCHMARK_SIGMA;

/// The offset from [`ICSBEP_HMF001_K`] this case produces, in pcm.
///
/// **−6 pcm, sem ±5, seed-to-seed sd 165 pcm** on the HIGH tier against
/// ENDF/B-VIII.0 — the pooled mean of **1024 independent seeds** at this
/// program's own settings (5000 histories × [40 inactive + 120 active], all
/// three ICSBEP nuclides, `CpuSingleThread` per seed — exactly this program's
/// run, repeated). Measured **2026-10-05** at commit `6faff1ed8` by
/// `examples/godiva_keff_ensemble.rs` (`OUTRAM_GODIVA_SEEDS=1024`), which
/// carries the method and the gates (GitHub #546).
///
/// ~~**+16 pcm, sem ±11, seed-to-seed sd 173 pcm**, 256 seeds, 2026-09-15~~ —
/// superseded by the line above; kept in the table below.
///
/// # The 2026-10-05 re-measure (GitHub #546)
///
/// **Why.** `+16 ± 11` was measured before URR and DBRC became defaults
/// (2026-09-20) and before the OpenMC-parity audit (2026-09-30, #407). The
/// five-route study then recorded route 4 — this model, on the multi-threaded
/// driver — at **−52 ± 27 pcm** over 32 seeds (2026-09-30, `0414bc8277`),
/// 2.3 σ from `+16`. A drift gate that may be stale has to be re-measured, not
/// argued about.
///
/// **Method.** `godiva_keff_ensemble.rs`, binary built at `6faff1ed8` (no
/// Godiva-relevant source change since: see the next paragraph), seeds
/// `1..=1024`. Size chosen from a timed pilot (4 seeds: 12.5 s of transport on
/// two workers): 1024 seeds is ~50 min, and at `sem ≈ 165/√1024 ≈ 5 pcm` the
/// comparison with `−52 ± 27` is limited by that record's own σ (combined
/// 27.5 pcm; 2048 seeds would only reach 27.3), so more seeds buy nothing here.
///
/// **Result.** `mean −6 pcm, sd 165, sem ±5` (`k = 0.99994 ± 0.00005`). Seed
/// consistency report: `χ²/dof = 0.97` (1023 dof, 95 % band 0.92–1.09), no
/// outlier seed, `1/√N` check over 256 groups of 4 = `1.01 ± 0.04`. Both of the
/// ensemble's gates pass (no regression from `+16`: 1.8 σ of 12 pcm;
/// benchmark: `|−6| ≤ 100`). Data processing 114 s, transport 2995 s, on an
/// Intel Xeon Processor @ 2.10 GHz, **2 of 4 shared logical cores**
/// (`taskset -c 2,3`, two single-threaded seed workers), 15.7 GB RAM, Linux
/// 6.18, CPU only; the machine was shared with another agent's runs and a
/// rust-analyzer process on one of the two cores.
///
/// **Against the records it is compared with.**
///
/// - `+16 ± 11` (256 seeds, 2026-09-15): **−22 ± 12 pcm (1.8 σ)**. Not
///   resolved as a move; several defaults changed in between (URR, DBRC, the
///   audit fixes) and their sum on Godiva is not decomposed here.
/// - Route 4, `−52 ± 27` (32 seeds): **+46 ± 27 pcm (1.7 σ)**. This is beyond
///   that record's σ, so it was checked rather than waved through: the route-4
///   driver at `8b17079bc` reproduces 0414bc8277's per-seed `k` for seeds 1–32
///   **to every printed digit (32/32)**, so the code did not move; and extended
///   to seeds 1–288 (multi-threaded, 2 threads, same hardware, 3.3 s per seed)
///   it gives **−10 ± 11 pcm (sd 183)**, seeds 33–288 alone −5 ± 12. The two
///   drivers draw different random streams per seed (single- vs
///   multi-threaded), so these are independent samples of one distribution;
///   they differ by −4 ± 12 pcm. **The 32-seed −52 was a low draw**
///   (−1.6 σ against the independent seeds 33–288), not a code effect. Prediction written
///   before the extension: "within 2 σ of −6, about [−27, +15]"; it held.
///
/// Gate width after re-pointing: `4·√(165² + 5²) ≈ 660 pcm` for one run of
/// this program — still set by the run's own noise.
///
/// # How this number got here
///
/// | recorded | what it was | how it moved |
/// |---|---|---|
/// | `+57 ± 173` | **one seed's draw**, −0.97 sigma of a `+228 ± 18 pcm` distribution | superseded, gh:#196 |
/// | `+228 ± 18` | the pre-gh:#192 code's true mean, 96 seeds | MT=91 Q-value cap: **+85 ± 26 pcm** |
/// | `+314 ± 21` | after the cap, 96 seeds | evaluated MF=6 law: **−105 ± 32 pcm** |
/// | `+214 ± 20` | 64 seeds, 2026-09-13 | discrete inelastic MF=4 angles (`op-tm9f`): **−198 pcm** |
/// | ~~`+16 ± 11`~~ | 256 seeds, 2026-09-15; **superseded 2026-10-05** | URR + DBRC defaults, the #407 audit: −22 ± 12 pcm (1.8 σ), not decomposed |
/// | **`−6 ± 5`** | **now**: 1024 seeds, 2026-10-05, `6faff1ed8` (#546) | — |
///
/// ~~**Not the latest pooled number (noted 2026-10-04, not re-measured here).**
/// The five-route study re-ran this model after the OpenMC-parity audit
/// (gh:#407) and records **0.99948 ± 0.00027, i.e. −52 ± 27 pcm** (32 seeds at
/// these settings, 2026-09-30, commit `0414bc8277`, route 4 of
/// `verification_and_validation/icsbep/five_route_keff_2026_09_29.md`), 2.3
/// sigma from `+16 ± 11`. That is the number to quote for the current code
/// (the tutorial does). This constant still holds `+16`: re-pointing a drift
/// gate is a re-measurement decision, and at this program's single-run
/// resolution (~613-693 pcm) the gate passes either way.~~ **RESOLVED
/// 2026-10-05 (#546):** re-measured and re-pointed, see the top of this
/// comment. The `−52 ± 27` was a 32-seed low draw; the number to quote is
/// `−6 ± 5`.
///
/// Two single runs of this program differ by ~√2 × 173 ≈ 245 pcm from
/// re-randomisation alone, whatever the physics does, which is why every entry
/// above is a pooled mean and none is a single run — **and why this program,
/// which takes exactly one draw, cannot itself establish any of them.** It is
/// checked *against* the pooled value, at the resolution one draw has.
///
/// # Why this constant now carries an uncertainty (gh:#196, 2026-09-16)
///
/// It is passed as `RecordedKeff::pooled(RECORDED_PCM, RECORDED_SEM_PCM)`
/// (~~`(16.0, 11.0)`~~ `(−6.0, 5.0)` since 2026-10-05), not as a bare number. The drift gate is the spread of the **difference of two
/// independent measurements**, `4·√(σ_run² + σ_recorded²)` — here
/// `4·√(173² + 11²) ≈ 693 pcm`, essentially set by this single run's own noise,
/// which is the honest resolution of a one-seed check. The previous helper
/// gated at `4·σ_run` and treated the recorded value as exact, too tight by up
/// to `√2`.
///
/// The `+214` this constant used to hold was **stale**: it predates `op-tm9f`,
/// which moved the case −198 pcm. A single run of this program would have had to
/// be ~1.2 σ low to notice, so the staleness was inside the old gate's noise —
/// exactly the failure mode a drift gate exists to catch, and did not.
///
/// The two physics changes of 2026-09-13 pull opposite ways, and both are right.
/// Capping the MT=91 outgoing energy at the two-body bound (gh:#192) moved this
/// case 85 pcm **further** from a measured criticality experiment. Replacing the
/// Weisskopf evaporation stand-in with the evaluation's own `f₀(E→E')` (ENDF
/// MF=6 LAW=1) moved it 105 pcm **back toward** it. Neither was chosen for its
/// direction.
///
/// This is checked as a **separate claim** from agreement with the benchmark.
/// A result can sit comfortably inside the ICSBEP band and still drift steadily
/// within it; only the reproduction claim sees that. If this fails, either the
/// physics moved — find out what — or this number is stale, in which case
/// update it here with the date and the reason rather than deleting the check.
///
/// Other sites in this repo still quote superseded values in their own
/// arguments (`jemima_keff.rs`, `hst009_keff.rs`, `lct008_keff.rs`,
/// `endf_to_keff.rs`, `verification_and_validation/ring_rpt/`, and the Part II
/// paper dataset). Sweeping them is the remainder of gh:#196 / `bn:op-awwi`,
/// and wants a pooled re-measurement of each of those cases rather than a
/// find-and-replace of this number.
const RECORDED_PCM: f64 = -6.0;
/// The `sem` on [`RECORDED_PCM`]: `165/√1024`, from the 1024-seed ensemble of
/// 2026-10-05 (~~`173/√256 = 11`~~ before).
const RECORDED_SEM_PCM: f64 = 5.0;
