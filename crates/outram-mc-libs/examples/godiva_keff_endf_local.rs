//! Godiva bare-sphere criticality on the **HIGH tier**, from the repo's own
//! ENDF/B-VIII.0 tapes — no network.
//!
//! # Why this exists
//!
//! The FHR pebble study (`verification_and_validation/ring_rpt/`) sits ~+1.7 %
//! above its OpenMC reference and every *data* mechanism has been excluded by
//! measurement: point σ vs NJOY's PENDF (±0.04 %), the capture **resonance
//! integral** vs NJOY and vs the published RI_∞ (+0.00 %,
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
//! history. That is the value [`RECORDED_PCM`] now holds, and
//! `examples/godiva_keff_ensemble.rs` is where it is measured and gated.
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

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
use outram_mc_libs::vv::{assert_reproduces_keff, RecordedKeff};
use std::time::Instant;

/// Godiva material temperature \[K\] (room temperature; the benchmark is a metal
/// assembly, not a reactor).
const TEMP_K: f64 = 293.6;

fn main() {
    println!("Reconstructing HEU isotopes from ENDF/B-VIII.0 (RECONR + BROADR @ {TEMP_K} K)…");
    let t0 = Instant::now();
    let nuclides: Vec<Nuclide> = [
        ("U234", "n-092_U_234-ENDF8.0.endf"),
        ("U235", "n-092_U_235-ENDF8.0.endf"),
        ("U238", "n-092_U_238.endf"),
    ]
    .iter()
    .map(|(name, file)| {
        let p = reference_endf(file).unwrap_or_else(|| panic!("missing reference tape {file}"));
        let t = Instant::now();
        let n = Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
            .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
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
    // three runs differ only in where σ comes from.
    let material = Material {
        id: 1,
        name: "Godiva HEU".into(),
        temperature: TEMP_K,
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.9184e-4,
            }, // U-234
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 4.4994e-2,
            }, // U-235
            NuclideComponent {
                nuclide_idx: 2,
                atom_density: 2.4984e-3,
            }, // U-238
        ],
    };

    let settings = KeffSettings {
        n_particles: 5000,
        n_inactive: 40,
        n_active: 120,
        temperature_k: TEMP_K,
        ..KeffSettings::default()
    };

    let radius_cm = 8.7407;
    println!("Godiva bare-sphere Keff — HIGH fidelity, ENDF/B-VIII.0  (r = {radius_cm} cm)");
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
    // The recorded result is **+214 ± 20 pcm**, the pooled mean of a 64-seed
    // ensemble of this program at these settings (2026-09-13); the single-seed
    // +57 it ultimately replaced turned out to be a −0.97 sigma draw. It is
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
const ICSBEP_HMF001_K: f64 = 1.0000;

/// The ICSBEP-stated uncertainty on [`ICSBEP_HMF001_K`].
const ICSBEP_HMF001_BAND: f64 = 0.0010;

/// The offset from [`ICSBEP_HMF001_K`] this case produces, in pcm.
///
/// **+16 pcm, sem ±11, seed-to-seed sd 173 pcm** on the HIGH tier against
/// ENDF/B-VIII.0 — the pooled mean of **256 independent seeds** at this
/// program's own settings (5000 histories × [40 inactive + 120 active], all
/// three ICSBEP nuclides, single-threaded CPU). Measured 2026-09-15 by
/// `examples/godiva_keff_ensemble.rs`, which carries the method and the gates.
///
/// # How this number got here
///
/// | recorded | what it was | how it moved |
/// |---|---|---|
/// | `+57 ± 173` | **one seed's draw**, −0.97 sigma of a `+228 ± 18 pcm` distribution | superseded, gh:#196 |
/// | `+228 ± 18` | the pre-gh:#192 code's true mean, 96 seeds | MT=91 Q-value cap: **+85 ± 26 pcm** |
/// | `+314 ± 21` | after the cap, 96 seeds | evaluated MF=6 law: **−105 ± 32 pcm** |
/// | `+214 ± 20` | 64 seeds, 2026-09-13 | discrete inelastic MF=4 angles (`op-tm9f`): **−198 pcm** |
/// | `+16 ± 11` | **now**, 256 seeds, 2026-09-15 | — |
///
/// Two single runs of this program differ by ~√2 × 173 ≈ 245 pcm from
/// re-randomisation alone, whatever the physics does, which is why every entry
/// above is a pooled mean and none is a single run — **and why this program,
/// which takes exactly one draw, cannot itself establish any of them.** It is
/// checked *against* the pooled value, at the resolution one draw has.
///
/// # Why this constant now carries an uncertainty (gh:#196, 2026-09-16)
///
/// It is passed as [`RecordedKeff::pooled(16.0, 11.0)`](RecordedKeff::pooled),
/// not as a bare number. The drift gate is the spread of the **difference of two
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
const RECORDED_PCM: f64 = 16.0;
/// The `sem` on [`RECORDED_PCM`]: `173/√256`, from the 256-seed ensemble.
const RECORDED_SEM_PCM: f64 = 11.0;
