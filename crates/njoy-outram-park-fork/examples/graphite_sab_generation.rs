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
//! Termux. **The graphite deck is embedded in this crate** via `include_str!`
//! (see `leapr::decks::embedded_deck_text`), so it runs with nothing else on
//! disk. `OUTRAM_PARK_TSL_DIR` remains a fallback for materials that are not
//! embedded, and `locate_deck` prefers the embedded copy when there is one.
//!
//! CORRECTION (2026-09-11): this paragraph used to say the crate "does not embed
//! one, because the redistribution terms of those files are unestablished". That
//! was true when it was written and is not true now — 33 decks are embedded,
//! graphite among them. The stale claim was load-bearing: `main` pre-checked the
//! filesystem candidates and returned early when none existed, so this program
//! silently skipped itself on every machine without an unpacked ENDF/B-VIII.0
//! tree, despite the deck being compiled into the binary. See
//! `docs/leapr-deck-provenance.md` for the provenance that was established.

use std::time::Instant;

use njoy_outram_park_fork::leapr::decks::{candidate_deck_paths, SabMaterial};
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

    // ── 1. Provenance: what the deck says about itself ───────────────────────
    //
    // `locate_deck` prefers the deck EMBEDDED in this crate and only falls back
    // to the filesystem candidates. This used to pre-check `candidate_deck_paths`
    // and return early when none of them existed — which skipped the whole
    // program on every machine without an unpacked ENDF/B-VIII.0 tree, even
    // though `embedded_deck_text(CrystallineGraphite)` has been `Some` all
    // along. The guard was checking the fallback and ignoring the primary.
    let located = match njoy_outram_park_fork::leapr::decks::locate_deck(material) {
        Ok(d) => d,
        Err(e) => {
            println!("No LEAPR deck for {}: {e}", material.label());
            println!(
                "\nThis is unexpected — {} is embedded in this crate via include_str!. \
                 \nIf you are seeing this, the embed has been removed.",
                material.label()
            );
            return;
        }
    };
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

    // ── 5. Parity against the official tape ──────────────────────────────────
    //
    // The oracle is the official ENDF/B-VIII.0 tape. It lives in this repo's own
    // `reference-data/endf/`, which is where every other V&V case in this
    // workspace reads its evaluations from — so the parity check is looked up
    // there FIRST and only then beside a filesystem deck.
    //
    // It used to be looked up beside the deck and nowhere else, so on a machine
    // using the EMBEDDED deck (which has no directory) there was nothing to look
    // beside and the parity check — the entire oracle comparison this program
    // exists for — skipped itself.
    let tape_name = format!("{}.endf", material.base());
    let tape_path =
        njoy_outram_park_fork::reference_data::reference_endf(&tape_name).or_else(|| {
            candidate_deck_paths(material)
                .iter()
                .find(|p| p.exists())
                .and_then(|p| p.parent().map(|d| d.join(&tape_name)))
                .filter(|p| p.exists())
        });
    let Some(tape_path) = tape_path else {
        println!(
            "\n(no {tape_name} in reference-data/endf/ and none beside a local deck; \
             skipping the parity check)"
        );
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

    // ── V&V gate, MT=4 ────────────────────────────────────────────────────────
    //
    // The oracle is the official ENDF/B-VIII.0 tape itself.
    //
    // CORRECTION (2026-09-11). This gate first asserted `identical == total` --
    // bit-for-bit on every stored value -- on the strength of the CONDITIONAL
    // print above it ("if identical == total { ... every stored S matches ... }").
    // That is an if-branch, not an invariant, and promoting it to one without
    // checking was the same mistake this whole sweep exists to correct: reading
    // a printed narrative as an established result. Measured, it is 6645 of
    // 60000, not all of them.
    //
    // What IS true is far stronger, and is what the gate asserts now: the
    // regenerated S agrees with the official tape to **1.004e-13 max, 9.991e-14
    // rms** over 48941 points above 1e-30 -- f64 round-off through the entire
    // LEAPR pipeline. At that level, whether a given stored value also happens
    // to round to identical ENDF sigfigs is an artefact of where the decimal
    // falls, not a physics claim, so the count is reported and not asserted.
    println!("\n=== V&V gate: regenerated MT=4 vs the official ENDF/B-VIII.0 tape ===");
    assert!(
        n > 1000,
        "only {n} S(alpha,beta) values above 1e-30 were compared against the \
         official tape. Graphite's MF=7/MT=4 is a 400 beta x 150 alpha grid; a \
         handful of points means the comparison collapsed, not that it passed."
    );
    let rms = (sumsq / n.max(1) as f64).sqrt();
    assert!(
        max < 1.0e-12,
        "the regenerated MT=4 S(alpha,beta) deviates from the official \
         ENDF/B-VIII.0 tape by up to {max:.3e} (rms {rms:.3e}) over {n} points \
         above 1e-30. Recorded: max 1.004e-13, rms 9.991e-14 -- f64 round-off \
         through the whole LEAPR pipeline.\n\
         A departure means a real change in the port or in the vintage-constant \
         selection. The deck's own EVAL-<MON><YY> card picks that constant set, \
         and getting it wrong costs a factor of ~100 on this channel, so what \
         this guards against is orders of magnitude, not digits."
    );
    println!(
        "  [PASS] regenerated MT=4 matches the official tape to {max:.3e} max, \
         {rms:.3e} rms over {n} points ({identical}/{total} stored values also \
         bit-identical after endout's sigfig rounding)"
    );
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

    // ── V&V gate, MT=2 ────────────────────────────────────────────────────────
    //
    // The coherent-elastic channel is ~90 % of graphite's thermal cross section
    // and it depends on a DIFFERENT combination of the vintage constants than
    // MT=4 does: the Bragg edges are `E = tau^2 / econ`, not `tev = bk*T`. So
    // MT=4 agreeing does not imply MT=2 agrees, and the two are gated
    // separately rather than as one "graphite matches" claim.
    println!("\n=== V&V gate: regenerated MT=2 (coherent elastic) vs the same tape ===");
    assert_eq!(
        ours_ce.bragg_energies_ev.len(),
        theirs_ce.bragg_energies_ev.len(),
        "the regenerated Bragg grid has {} edges against the tape's {}. The edge \
         COUNT is set by the lattice sum cutoff, so a mismatch here is a \
         different lattice, not a numerical difference — and the per-edge \
         comparison below would be silently comparing mismatched pairs.",
        ours_ce.bragg_energies_ev.len(),
        theirs_ce.bragg_energies_ev.len(),
    );
    // Edge energies are geometry (tau^2/econ) and carry no Debye-Waller factor,
    // so they are held far tighter than S(E), which does.
    assert!(
        e_max < 1.0e-6,
        "Bragg edge energies deviate by up to {e_max:.3e} from the official tape. \
         These are `E = tau^2 / econ` — pure lattice geometry and one vintage \
         constant — so they should agree to round-off. A departure means `econ` \
         (not `bk`) has been selected wrongly, which MT=4 would not notice."
    );
    assert!(
        s_max < 1.0e-4,
        "coherent-elastic S(E) deviates by up to {s_max:.3e} from the official \
         tape at 296 K. This channel is ~90 % of graphite's thermal cross \
         section, so an error here moves the moderator's interaction rate \
         directly."
    );
    println!(
        "  [PASS] {} Bragg edges, max rel dev {e_max:.3e} on edge energies and \
         {s_max:.3e} on S(E)",
        ours_ce.bragg_energies_ev.len()
    );
}
