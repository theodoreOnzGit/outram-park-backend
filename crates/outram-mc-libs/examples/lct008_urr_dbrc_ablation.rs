// SPDX-License-Identifier: GPL-3.0

//! **Is the ACE-vs-ENDF `k` agreement real, or is it two effects cancelling?**
//! — GitHub #307.
//!
//! `examples/lct008_ace_roundtrip.rs` reports the two data routes agreeing at
//! **0.78 sigma** on a homogenised LCT-008:
//!
//! ```text
//! ENDF route : k_eff = 0.84980 +/- 0.00232
//! ACE  route : k_eff = 0.85250 +/- 0.00258
//! difference : +269.3 pcm  (combined sigma 346.6 pcm, 0.78 sigma)  -> AGREE
//! ```
//!
//! That example already records the asymmetry behind the comparison:
//! `Nuclide::from_ace` sets `urr: None` and `dbrc: None`, while
//! `Nuclide::from_endf_file` applies **both** by default since the 2026-09-20
//! "correct physics is the DEFAULT SETTING" change. So the ENDF arm carries
//! unresolved-resonance self-shielding and DBRC and the ACE arm carries neither.
//!
//! **The two routes are therefore not a format comparison — they differ in
//! physics too, and the 0.78 sigma cannot tell the two apart.** This example
//! separates them: it ablates URR and DBRC *on the ENDF arm* and measures how
//! far that moves `k`.
//!
//! # Why this is the right instrument
//!
//! It is a **paired** ablation — same nuclides, same geometry, same material,
//! same seed per pair, one thing changed — so it measures the worth of URR+DBRC
//! directly instead of inferring it from the difference of two independently
//! noisy numbers. The 0.78 sigma above rests on a combined sigma of 347 pcm and
//! cannot resolve a 269 pcm effect; a paired ensemble over `N` seeds shrinks the
//! uncertainty on the *difference* as `1/sqrt(N)` and can.
//!
//! # The prediction, recorded BEFORE the run
//!
//! Stated so the measurement is capable of failing:
//!
//! - **DBRC** treats target motion in resonance elastic scattering, which
//!   raises U-238 capture, so removing it should raise `k` (worth of order
//!   −200 to −400 pcm in LEU lattices, i.e. the ablation is positive).
//! - **URR probability tables** self-shield the unresolved range, lowering
//!   effective absorption, so removing them should lower `k` (of order +50 to
//!   +200 pcm, i.e. the ablation is negative).
//! - They **oppose**, with DBRC normally the larger, so the net ablation was
//!   predicted at **+100 to +300 pcm** — closing most of the observed
//!   +269 pcm gap.
//!
//! **If the ablated arm does not move within statistics**, then URR+DBRC are
//! not the cause, this homogenised case is insensitive to them, and the
//! 0.78 sigma stands as a (weak) format-precision check. That is a real
//! possible outcome, not a formality: homogenising the lumped fuel destroys
//! precisely the self-shielding that makes the resonance treatments matter, so
//! a null result is physically plausible here even though the same ablation
//! would bite in the true lattice.
//!
//! # Results, 2026-09-25 — a BOUND, not a measurement
//!
//! Twelve seeds, 3000 histories x [30 inactive + 120 active]:
//!
//! ```text
//! k with URR+DBRC : 0.84937  (seed-to-seed sd 0.00181)
//! k ablated       : 0.85001  (seed-to-seed sd 0.00222)
//! ABLATION WORTH  : +63.5 pcm, sem 77.2, sd 267.5  (0.8 sigma)
//! ```
//!
//! **NOT RESOLVED at 2 sigma**, so the result is `|worth| < 154 pcm at
//! 2 sigma`. Do not quote `+63.5 pcm` as the worth.
//!
//! **The prediction above is NOT supported.** `+100 to +300 pcm` was predicted
//! and `+63.5 ± 77` measured: the sign is right, the magnitude is below the
//! range, and zero is not excluded. Recorded as a miss.
//!
//! **And the gap being explained was itself only 0.78 sigma** — never
//! established to exist. The bound here and that non-significance are jointly
//! consistent with there being no route difference and no URR+DBRC worth on
//! this homogenised model.
//!
//! **Two methodological findings.** The pairing bought nothing: sd on the
//! difference is 267.5 against per-arm sds of 181/222, i.e. `sqrt(2) x ~200`,
//! the uncorrelated result — URR and DBRC change how many draws each history
//! consumes, so the streams diverge and the arms do not correlate. Size this as
//! independent arms, or give URR its own `future_seed` substream. Resolving
//! `+63.5` at 3 sigma needs ~162 seeds.
//!
//! **Do not spend that here.** This model homogenises the lumped fuel, which is
//! what destroys the self-shielding that makes these treatments matter. The
//! number worth having belongs on the lumped case (`lct008_keff.rs`).
//!
//! Full record:
//! `verification_and_validation/ace_route_physics/urr_dbrc_worth_2026_09_25.md`.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example lct008_urr_dbrc_ablation
//! ```
//!
//! Override the seed count with `OUTRAM_ABLATION_SEEDS` (default 12) and the
//! histories per generation with `OUTRAM_ABLATION_PARTICLES` (default 3000,
//! matching the round-trip example).

use std::time::Instant;

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{run_keff, KeffSettings};

const TEMP_K: f64 = 293.6;
const FUEL_VF: f64 = 0.30;
const WATER_VF: f64 = 1.0 - FUEL_VF;
const RADIUS_CM: f64 = 40.0;

/// The observed ACE − ENDF difference this example exists to explain.
///
/// ~~`269.3` pcm, one seed~~ **CORRECTED 2026-09-25 to `23.9` pcm, eight seeds**
/// (GitHub #307 item 5,
/// `verification_and_validation/ace_route_physics/route_parity_8seed_2026_09_25.md`).
/// `lct008_ace_roundtrip.rs` now runs the comparison over eight seeds and gets
/// `+23.9 ± 125.0 pcm` against a per-seed spread of ~250 pcm, so the `+269.3`
/// this example was built to explain was **one seed's fluctuation of about one
/// standard deviation**. There was never a 269 pcm gap to attribute.
///
/// The constant is kept, and the percentage printed from it is kept, because the
/// comparison is still the right one to show — but it now compares a bound
/// against a bound, and the output says so. Deleting it would hide that this
/// example's premise moved.
const OBSERVED_GAP_PCM: f64 = 23.9;

/// Same five nuclides, densities and volume fractions as
/// `lct008_ace_roundtrip.rs`, so the two examples describe the same model.
const NUCLIDES: [(&str, &str, f64, f64); 5] = [
    ("U235", "n-092_U_235-ENDF8.0.endf", 0.00056868, 0.0),
    ("U238", "n-092_U_238.endf", 0.022268, 0.0),
    ("O16", "n-008_O_016-ENDF8.0.endf", 0.045683, 0.033369),
    ("H1", "n-001_H_001-ENDF8.0-Beta6.endf", 0.0, 0.066737),
    ("B10", "n-005_B_010-ENDF8.0.endf", 2.6055e-07, 1.6769e-05),
];

fn mean_sd(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    if v.len() < 2 {
        return (m, 0.0);
    }
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0);
    (m, var.sqrt())
}

fn main() {
    let seeds: usize = std::env::var("OUTRAM_ABLATION_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);
    let particles: usize = std::env::var("OUTRAM_ABLATION_PARTICLES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);

    println!("LCT-008 (homogenised): the worth of URR + DBRC on the ENDF route");
    println!(
        "  {seeds} paired seeds x {particles} histories, r = {RADIUS_CM} cm, \
         {:.0} % fuel / {:.0} % borated water",
        100.0 * FUEL_VF,
        100.0 * WATER_VF
    );
    println!(
        "  explaining the ACE-minus-ENDF gap of {OBSERVED_GAP_PCM:+.1} pcm from \
         lct008_ace_roundtrip.rs\n"
    );

    // ── Build once, then ablate a copy ─────────────────────────────────────
    let t = Instant::now();
    let mut full: Vec<Nuclide> = Vec::new();
    for (name, file, _, _) in NUCLIDES {
        let Some(path) = reference_endf(file) else {
            println!("SKIP: reference tape {file} is not present");
            return;
        };
        let n = Nuclide::from_endf_file(&path, name, TEMP_K, 1.0e-3)
            .unwrap_or_else(|e| panic!("from_endf_file({name}): {e}"));
        full.push(n);
    }
    println!("  built 5 nuclides from ENDF in {:.1} s", t.elapsed().as_secs_f64());

    let ablated: Vec<Nuclide> = full
        .iter()
        .cloned()
        .map(|n| n.without_urr_probability_tables().without_dbrc())
        .collect();

    // **THE ABLATION CONTROL.** An ablation that removes nothing measures
    // nothing, and this workspace has shipped one before (`op-rbo`). Assert the
    // two arms genuinely differ, and say on which nuclides, before spending an
    // hour of transport on them.
    let mut differing = Vec::new();
    for (i, (name, ..)) in NUCLIDES.iter().enumerate() {
        let (fu, fd) = (full[i].has_urr_probability_tables(), full[i].has_dbrc());
        let (au, ad) = (ablated[i].has_urr_probability_tables(), ablated[i].has_dbrc());
        println!("    {name:5} full: urr={fu:5} dbrc={fd:5}   ablated: urr={au:5} dbrc={ad:5}");
        if (fu && !au) || (fd && !ad) {
            differing.push(*name);
        }
    }
    assert!(
        !differing.is_empty(),
        "the ablation removed NOTHING from any nuclide: both arms are identical, so \
         any difference measured below would be pure noise and any null result would \
         be vacuous. Check that from_endf_file still attaches URR/DBRC by default."
    );
    println!("  ablation is real on: {differing:?}\n");

    let components: Vec<NuclideComponent> = NUCLIDES
        .iter()
        .enumerate()
        .map(|(i, (_, _, fuel_ao, water_ao))| NuclideComponent {
            nuclide_idx: i,
            atom_density: fuel_ao * FUEL_VF + water_ao * WATER_VF,
        })
        .collect();
    let material = |name: &str| Material {
        id: 1,
        name: name.into(),
        temperature: TEMP_K,
        components: components.clone(),
    };

    // ── Paired arms ────────────────────────────────────────────────────────
    let mut k_full = Vec::with_capacity(seeds);
    let mut k_abl = Vec::with_capacity(seeds);
    let mut diffs = Vec::with_capacity(seeds);

    println!("  seed    k(URR+DBRC)      k(ablated)      diff [pcm]");
    for s in 0..seeds {
        let settings = KeffSettings {
            n_particles: particles,
            n_inactive: 30,
            n_active: 120,
            temperature_k: TEMP_K,
            seed: 1 + s as u64,
            ..KeffSettings::default()
        };
        let a = run_keff(RADIUS_CM, &material("full"), &full, &settings);
        let b = run_keff(RADIUS_CM, &material("ablated"), &ablated, &settings);
        let d = 1.0e5 * (b.k_mean - a.k_mean);
        println!(
            "  {:4}    {:.5}         {:.5}        {d:+8.1}",
            settings.seed, a.k_mean, b.k_mean
        );
        k_full.push(a.k_mean);
        k_abl.push(b.k_mean);
        diffs.push(d);
    }

    // ── The measurement ────────────────────────────────────────────────────
    let (m_full, sd_full) = mean_sd(&k_full);
    let (m_abl, sd_abl) = mean_sd(&k_abl);
    let (m_d, sd_d) = mean_sd(&diffs);
    let sem_d = if seeds > 1 {
        sd_d / (seeds as f64).sqrt()
    } else {
        0.0
    };
    let sigma = if sem_d > 0.0 { m_d.abs() / sem_d } else { 0.0 };

    println!("\n  RESULT over {seeds} paired seeds");
    println!(
        "    k with URR+DBRC : {m_full:.5}  (seed-to-seed sd {:.5})",
        sd_full
    );
    println!(
        "    k ablated       : {m_abl:.5}  (seed-to-seed sd {:.5})",
        sd_abl
    );
    println!(
        "    ABLATION WORTH  : {m_d:+.1} pcm, sem {sem_d:.1}, sd {sd_d:.1}  ({sigma:.1} sigma)"
    );
    println!(
        "    observed ACE-ENDF gap : {OBSERVED_GAP_PCM:+.1} pcm (8 seeds, +/- 125.0)  \
         => URR+DBRC explains {:.0} % of it",
        100.0 * m_d / OBSERVED_GAP_PCM
    );
    println!(
        "    BOTH SIDES OF THAT RATIO ARE UNRESOLVED, so the percentage is arithmetic\n             and not an attribution: the gap is {OBSERVED_GAP_PCM:+.1} +/- 125.0 pcm (0.19\n             sigma) and the worth below is quoted with its own sigma. The single-seed\n             +269.3 pcm this example was built to explain was one seed's fluctuation --\n             see route_parity_8seed_2026_09_25.md."
    );

    println!("\n  READING THE OUTCOME");
    if sigma < 2.0 {
        println!(
            "    NOT RESOLVED at 2 sigma. The honest statement is a BOUND, not a\n    \
             measurement: |worth| < {:.0} pcm at 2 sigma. On this evidence URR+DBRC\n    \
             are not shown to explain the {OBSERVED_GAP_PCM:+.1} pcm gap, and the\n    \
             0.78 sigma route agreement is not shown to be an accident either.\n    \
             Do NOT quote {m_d:+.1} pcm as the worth.",
            2.0 * sem_d
        );
    } else if (m_d - OBSERVED_GAP_PCM).abs() <= 2.0 * sem_d {
        println!(
            "    The ablation reproduces the gap within 2 sigma. The ACE-vs-ENDF\n    \
             'agreement' at 0.78 sigma is then largely a PHYSICS difference, not a\n    \
             format one, and must not be cited as route equivalence until the ACE\n    \
             route carries URR+DBRC (#307)."
        );
    } else {
        println!(
            "    Resolved, but it does NOT account for the gap ({m_d:+.1} pcm against\n    \
             {OBSERVED_GAP_PCM:+.1} pcm). Part of the difference is URR+DBRC and part is\n    \
             something else -- the ACE writer/reader path is the next suspect."
        );
    }
    println!(
        "\n    Caveat that belongs with any reading: this model HOMOGENISES the lumped\n    \
         fuel that gives LCT-008 its self-shielding, which is exactly what makes the\n    \
         resonance treatments matter. A small worth here does not mean a small worth\n    \
         in the true lattice."
    );
}
