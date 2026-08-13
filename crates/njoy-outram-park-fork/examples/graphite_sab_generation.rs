//! Regenerate graphite `S(alpha, beta)` from its LEAPR deck — the default path.
//!
//! Run it:
//!
//! ```bash
//! OUTRAM_PARK_TSL_DIR=/path/to/ENDF-B-VIII.0/thermal_scatt \
//!   cargo run --release -p njoy-outram-park-fork --example graphite_sab_generation
//! ```
//!
//! What it shows, in order:
//!
//! 1. **Where the deck came from and which physical constants it selects.** The
//!    deck's own `EVAL-<MON><YY>` comment card picks the constant set; for
//!    ENDF/B-VIII.0 graphite that is `EVAL-SEP17`, which selects NJOY's
//!    pre-2018 constants (`bk = 8.617385e-5 eV/K` and four more). Getting this
//!    wrong costs a factor of ~100 on the inelastic channel and ~1e7 on the
//!    elastic one, so it is printed rather than hidden.
//! 2. **A cold generation, timed** — the ~seconds-per-temperature cost.
//! 3. **A warm call, timed** — served from the in-process memo.
//! 4. **Two temperatures the ENDF tape does not tabulate** (HTR-10's 393 K and
//!    523 K), which is the reason to keep the deck rather than the tape.
//! 5. **An MF=7/MT=4 parity check against the official tape**, if one is
//!    present next to the deck — stored value by stored value.
//! 6. **The same for MF=7/MT=2**, the coherent-elastic channel, which is ~90 %
//!    of graphite's thermal cross section and depends on a *different*
//!    combination of the vintage constants (`econ`, not `bk`).
//!
//! Run it twice: the second run skips generation entirely and reads the
//! artifact the first run left in the on-disk cache.
//!
//! This example needs no network and no GPU, and builds and runs on Android /
//! Termux. It needs a local copy of the ENDF/B-VIII.0 `.leapr` deck — this
//! crate does not embed one, because the redistribution terms of those files
//! are unestablished (see `leapr::decks::embedded_deck_text` and
//! `docs/leapr-deck-provenance.md`). Without it, the example prints what to set
//! and exits successfully.

use std::time::Instant;

use njoy_outram_park_fork::leapr::decks::{candidate_deck_paths, SabMaterial, TSL_DIR_ENV};
use njoy_outram_park_fork::leapr::generate::{thermal_scattering_law, SabRequest};
use njoy_outram_park_fork::thermr::mf7::parse_mf7_at_temperature;
use njoy_outram_park_fork::units::Temperature;
use uom::si::thermodynamic_temperature::kelvin;

/// Ask for one law and report how long it took and what came back.
fn timed(
    label: &str,
    request: &SabRequest,
) -> Option<std::sync::Arc<njoy_outram_park_fork::thermr::mf7::Mf7>> {
    let t0 = Instant::now();
    match thermal_scattering_law(request) {
        Ok(law) => {
            let dt = t0.elapsed().as_secs_f64();
            let (mt4, mt2) = request.validation();
            let n_beta = law
                .incoherent_inelastic
                .as_ref()
                .map(|ii| ii.beta.len())
                .unwrap_or(0);
            let n_alpha = law
                .incoherent_inelastic
                .as_ref()
                .and_then(|ii| ii.s_tables.first().map(|a| a.alpha.len()))
                .unwrap_or(0);
            let n_edges = law
                .coherent_elastic
                .as_ref()
                .map(|ce| ce.bragg_energies_ev.len())
                .unwrap_or(0);
            println!(
                "  {label:<34} {dt:>7.3} s   MT=4 {n_alpha} x {n_beta} [{mt4:?}]   \
                 MT=2 {n_edges} Bragg points [{mt2:?}]"
            );
            Some(law)
        }
        Err(e) => {
            println!("  {label:<34} FAILED: {e}");
            None
        }
    }
}

fn main() {
    println!("graphite S(alpha, beta) by LEAPR regeneration\n");

    let material = SabMaterial::CrystallineGraphite;
    let paths = candidate_deck_paths(material);
    if !paths.iter().any(|p| p.exists()) {
        println!(
            "No LEAPR deck for {}. Set {TSL_DIR_ENV} to the thermal_scatt/ directory\n\
             of an unpacked ENDF/B-VIII.0 distribution. Searched:",
            material.label()
        );
        for p in &paths {
            println!("  {}", p.display());
        }
        return;
    }

    // ── 1. Provenance: what the deck says about itself ───────────────────────
    let located = njoy_outram_park_fork::leapr::decks::locate_deck(material)
        .expect("a deck exists — checked above");
    let deck =
        njoy_outram_park_fork::leapr::deck::LeaprDeck::parse(&located.text).expect("deck parses");
    println!("deck      : {}", located.source);
    println!(
        "evaluation: {}  ->  constants {:?} (bk = {:e} eV/K)",
        deck.evaluation_date()
            .map(|d| d.to_string())
            .unwrap_or_else(|| "no EVAL- field".to_string()),
        deck.constants(),
        deck.constants().bk_ev_per_k()
    );
    println!(
        "grids     : {} alpha x {} beta, nphon = {}, tabulated at {:?} K\n",
        deck.alpha.len(),
        deck.beta.len(),
        deck.nphon,
        deck.temperatures_k()
    );

    // ── 2/3. Cold generation, then the same request again ────────────────────
    println!("timings (first run generates; re-run this example for a disk-cache hit)");
    let at_296 = SabRequest::new(material, Temperature::new::<kelvin>(296.0));
    let law_296 = timed("296 K (first call)", &at_296);
    timed("296 K (repeat, in-process memo)", &at_296);

    // ── 4. Temperatures the tape does not tabulate ───────────────────────────
    for t_k in [393.0, 523.0] {
        timed(
            &format!("{t_k} K (untabulated, HTR-10)"),
            &SabRequest::new(material, Temperature::new::<kelvin>(t_k)),
        );
    }

    // ── 5. Parity against the official tape, when it is available ────────────
    let tape_path = paths.iter().find(|p| p.exists()).and_then(|p| {
        p.parent()
            .map(|d| d.join(format!("{}.endf", material.base())))
    });
    let Some(tape_path) = tape_path.filter(|p| p.exists()) else {
        println!("\n(no reference tape beside the deck; skipping the parity check)");
        return;
    };
    let Some(law) = law_296 else { return };
    let Some(ours) = law.incoherent_inelastic.as_ref() else {
        return;
    };

    let tape = njoy_outram_park_fork::endf::tape::Tape::read(
        std::fs::File::open(&tape_path).expect("tape opens"),
    )
    .expect("tape parses");
    let theirs = parse_mf7_at_temperature(&tape, material.mat(), Some(296.0))
        .expect("MF=7 parses")
        .incoherent_inelastic
        .expect("graphite has MT=4");

    let (mut max, mut sumsq, mut n) = (0.0f64, 0.0f64, 0usize);
    let (mut total, mut identical) = (0usize, 0usize);
    for (a, b) in ours.s_tables.iter().zip(theirs.s_tables.iter()) {
        for (&x, &y) in a.s.iter().zip(b.s.iter()) {
            total += 1;
            if x.to_bits() == y.to_bits() {
                identical += 1;
            }
            if y <= 1e-30 {
                continue;
            }
            let rel = (x - y).abs() / y;
            max = max.max(rel);
            sumsq += rel * rel;
            n += 1;
        }
    }
    println!(
        "\nMT=4 vs {} at 296 K:\n  \
         max rel dev {:.3e}, rms {:.3e} over {n} points above 1e-30\n  \
         bit-identical stored values: {identical} / {total}",
        tape_path.display(),
        max,
        (sumsq / n.max(1) as f64).sqrt()
    );
    if identical == total {
        println!(
            "  -> every stored S matches the official tape exactly. `endout` applies the same\n     \
             sigfig rounding NJOY does, so the round-off residual the raw-kernel parity test\n     \
             measures (4.917e-6) vanishes once the value is written in ENDF form."
        );
    }
    // ── 6. The elastic channel, at the same temperature ──────────────────────
    //
    // MT=2 is ~90 % of graphite's thermal cross section. Its Bragg edge energies
    // are `E = tau^2 / econ`, so they depend on the vintage through a different
    // combination of constants than MT=4's `tev = bk*T` — see `leapr::vintage`.
    let full =
        njoy_outram_park_fork::thermr::mf7::parse_mf7(&tape, material.mat()).expect("MF=7 parses");
    let (Some(ours_ce), Some(theirs_ce)) = (
        law.coherent_elastic.as_ref(),
        full.coherent_elastic.as_ref(),
    ) else {
        return;
    };
    let i296 = theirs_ce
        .temperatures_k
        .iter()
        .position(|&t| t == 296.0)
        .expect("296 K tabulated");

    let rel = |a: f64, b: f64| if b == 0.0 { 0.0 } else { (a - b).abs() / b };
    let e_max = ours_ce
        .bragg_energies_ev
        .iter()
        .zip(theirs_ce.bragg_energies_ev.iter())
        .map(|(&a, &b)| rel(a, b))
        .fold(0.0f64, f64::max);
    let s_max = ours_ce.s_tables[0]
        .iter()
        .zip(theirs_ce.s_tables[i296].iter())
        .map(|(&a, &b)| rel(a, b))
        .fold(0.0f64, f64::max);
    println!(
        "\nMT=2 vs the same tape at 296 K:\n  \
         {} of {} Bragg grid points, max rel dev {:.3e} on edge energies, {:.3e} on S(E)",
        ours_ce.bragg_energies_ev.len(),
        theirs_ce.bragg_energies_ev.len(),
        e_max,
        s_max
    );
    println!(
        "  (all ten temperatures are covered by \
         tests/leapr_graphite_coherent_elastic_parity.rs)"
    );
}
