//! V&V: regenerate graphite `S(alpha, beta)` from its 12 KB LEAPR card deck and
//! compare against the 8.7 MB ENDF/B-VIII.0 tape that deck produced.
//!
//! # What this validates
//!
//! The end-to-end LEAPR *generation* path — [`LeaprDeck::parse`] ->
//! [`FrequencyModel::start`] -> [`phonon_expansion`] -> the `endout` symmetric-S
//! conversion — against the incoherent-inelastic MF=7/MT=4 section of the
//! official ENDF/B-VIII.0 crystalline-graphite evaluation. Until now the LEAPR
//! port had only closed-form and self-consistency checks (see
//! `src/leapr/README.md`); this is the first comparison against a reference
//! LEAPR tape, and it is the check that decides whether this workspace can ship
//! the 12 KB deck instead of the 8.7 MB tape.
//!
//! The point-by-point `S` comparison covers MT=4 only (the phonon-expansion
//! inelastic law). MT=2 is touched in one narrower way: the
//! `W' = lambda / (awr * T * k_B)` Debye-Waller conversion that `endout` expects
//! pre-applied, and that no code path in this crate currently performs, is
//! checked against the tape's own temperature trend in
//! [`debye_waller_conversion_matches_the_tape_temperature_trend`]. The Bragg
//! edge *positions* and structure factors from `coher` are **not** validated
//! here — they are validated in `tests/leapr_graphite_coherent_elastic_parity.rs`
//! (added 2026-08-13), which does the point-by-point MT=2 comparison this file
//! deliberately left out.
//!
//! [`every_endf_b_viii_leapr_deck_parses`] additionally runs the deck reader
//! over all 33 `tsl-*.leapr` files ENDF/B-VIII.0 ships, which is what surfaced
//! the `r*v` repeat counts and the undocumented `iel = -1` that graphite alone
//! would not have exposed.
//!
//! ## Data / provenance
//!
//! - **Deck:** `tsl-crystalline-graphite.leapr` (12,444 bytes), the LEAPR job
//!   distributed with the ENDF/B-VIII.0 thermal-scattering sublibrary (2018;
//!   open-source, per `DATA_POLICY.md`). Evaluated by the Low Energy
//!   Interaction Physics group at North Carolina State University
//!   (A.I. Hawari, Y. Zhu, J.L. Wormald, EVAL-SEP17) by ab-initio lattice
//!   dynamics. Cards: `nalpha = 150`, `nbeta = 400`, `lat = 1`, `nphon = 100`,
//!   a 201-point `rho(E)` on a 1 meV grid, `twt = c = 0`, `tbeta = 1`, `nd = 0`,
//!   `iel = 1` (graphite), and ten temperatures (296 K plus nine
//!   negative-signed "reuse the spectrum" entries).
//! - **Reference tape:** `tsl-crystalline-graphite.endf` (8,730,804 bytes),
//!   MAT 30, MF=7/MT=4, `LAT = 1`, `LASYM = 0`, `B = [4.73918, 197.6285,
//!   11.898, 5.000001, 0, 1]`.
//! - **Kernel:** port of NJOY2016 `leapr.f90` (commit `ac5adf5f`).
//! - Neither file is checked in. Tests read them from `GRAPHITE_TSL_DIR` (env
//!   override) or the default directory below, and **skip** (print a note, pass)
//!   when absent, so the suite stays green on a machine without the data.
//!
//! # Methodology
//!
//! 1. Parse the deck. `LeaprDeck::input_at(0)` yields the 296 K job.
//! 2. Run `FrequencyModel::start` on the deck's `rho(E)`, then
//!    `phonon_expansion` over the deck's own 150 x 400 alpha/beta grid at
//!    `nphon = 100`.
//! 3. Convert the LEAPR working array to the quantity the tape stores. For
//!    `LASYM = 0` / `LLN = 0`, `endout` (leapr.f90:3338-3346) writes the
//!    symmetric law `S = ssm(beta, alpha) * exp(-beta_scaled / 2)`, where
//!    `beta_scaled = beta * 0.0253 / (k_B T)` because `lat = 1`.
//! 4. Parse MF=7/MT=4 at 296 K with the THERMR reader and compare point by
//!    point on the shared 150 x 400 grid.
//! 5. **Pass criterion:** max relative deviation `< 1e-5` and RMS `< 2e-6` over
//!    every point where the tape value exceeds `1e-30`, when the generator is
//!    run with the Boltzmann constant NJOY carried at evaluation time (see
//!    below); and the set of exactly-zero points must match the tape's exactly.
//!
//! ## The Boltzmann-constant caveat (a real, measured effect)
//!
//! `S(alpha, beta)` depends on `k_B` through `tev = k_B T`, which sets both the
//! phonon grid spacing `deltab = delta_E / tev` and the `lat = 1` scale factor
//! `0.0253 / tev`. NJOY2016 carried `bk = 8.617385e-5 eV/K` until 23 Oct 2017,
//! then CODATA2014 `8.617342e-5`, and now CODATA2018 `8.617333262e-5` — which is
//! what this crate uses (`common::phys::BK_EV_PER_K`). The graphite evaluation
//! is dated SEP17, i.e. it predates the first of those changes.
//!
//! This is not a free parameter being tuned: the constant is an *input* the
//! oracle was run with, and matching it is part of reproducing the oracle. The
//! test measures the agreement **both** ways and asserts both, so the
//! as-shipped (CODATA2018) number is on the record too.
//!
//! Since 2026-08-13 the constant set is a **named field** on `LeaprInput`
//! (`njoy_outram_park_fork::leapr::vintage::PhysicalConstants`), selected
//! automatically from the deck's own `EVAL-SEP17` comment card, so this test
//! sets `input.constants` directly. It previously reproduced the historical
//! `tev` through an equivalent temperature `T' = T * bk_njoy / bk_crate`
//! (296.0017772 K standing in for 296 K); that workaround is gone, and the
//! measured figures are unchanged by its removal.
//!
//! # Measured results (2026-08-13, ENDF/B-VIII.0, release mode)
//!
//! Every number below was produced by running these tests on the actual files
//! on 2026-08-13. At 296 K, 48,941 of the 60,000 grid points carry a tape value
//! above `1e-30`; the other 11,059 are the deep tail, 6,645 of them stored as
//! exactly zero — a set this port reproduces **exactly**, same 6,645 points,
//! zero mismatches either way.
//!
//! | `bk` used to generate | max rel. dev. | RMS rel. dev. |
//! |---|---|---|
//! | `8.617385e-5` (NJOY at EVAL-SEP17) | **4.917e-6** | **6.390e-7** |
//! | `8.617342e-5` (CODATA2014) | 3.086e-4 | 5.687e-5 |
//! | `8.617333262e-5` (CODATA2018, crate default) | 3.711e-4 | 6.842e-5 |
//!
//! At 1000 K — a tabulated temperature reached through the deck's negative-
//! temperature spectrum reuse — the era-constant agreement is max 4.838e-6,
//! RMS 5.817e-7 over 52,378 compared points, again with an exactly matching
//! zero pattern (6,328 points). See [`sab_parity_against_endf_tape_at_1000k`].
//!
//! **The residual is the tape's own printed precision, not a physics
//! discrepancy — and it is measurably so, not merely plausibly so.** `endout`
//! rounds each value with `sigfig(x, 7, 0)` when it is `>= 1e-9` and
//! `sigfig(x, 6, 0)` below that (leapr.f90:3341-3345). Rounding to `n`
//! significant figures caps the relative error at `0.5 * 10^(1-n) / m` for a
//! mantissa `m` in `[1, 10)`, i.e. a worst case (at `m -> 1`) of **5e-7** for
//! 7 figures and **5e-6** for 6. Split at that `1e-9` boundary, the measured
//! 296 K deviations are:
//!
//! | Storage band | points | max rel. dev. | quantisation ceiling | RMS |
//! |---|---|---|---|---|
//! | `>= 1e-9`, 7 sig. figs | 37,950 | 4.948e-7 | 5.0e-7 | 1.355e-7 |
//! | `< 1e-9`, 6 sig. figs | 10,991 | 4.917e-6 | 5.0e-6 | 1.325e-6 |
//!
//! Each band's worst point sits at 99 % of its own storage ceiling and neither
//! exceeds it, and the two bands differ by exactly the factor of ten between
//! their formats. That is the signature of round-off in the reference file. A
//! genuine error in the phonon expansion would not respect a boundary defined
//! purely by how the number was *printed*.
//!
//! # Interpretation
//!
//! The LEAPR phonon-expansion path is **verified against ENDF/B-VIII.0** for
//! graphite: given the same deck and the same physical constants, it reproduces
//! the official tape to the tape's own printed precision. The 12 KB deck is
//! therefore a sufficient substitute for the 8.7 MB tape for the inelastic
//! channel, and generating at an untabulated temperature (e.g. HTR-10's 393 K
//! or 523 K) is a supported use of the same code path rather than an
//! extrapolation of it — subject to the physics caveat documented on
//! [`LeaprDeck::input_at_temperature`], namely that `rho(E)` itself is treated
//! as temperature-independent.
//!
//! Scope limits, stated plainly: this validates MT=4 only, for one moderator,
//! with `twt = c = 0` and `nd = 0` — so the translational, diffusive,
//! discrete-oscillator and cold-hydrogen branches of LEAPR are **not** touched
//! by it. The MT=2 elastic output is out of scope *here* but no longer
//! unvalidated: `tests/leapr_graphite_coherent_elastic_parity.rs` (2026-08-13)
//! compares the regenerated coherent-elastic section against the same tape
//! point by point, at all ten temperatures, and finds max 9.986e-7 — exact to
//! the last printed digit once the evaluation's own `econ` constants are used.
//! It also pins the *absolute* `W'` scale to 4.91e-7, which the temperature-
//! trend method used below explicitly could not do.
//!
//! [`LeaprDeck::parse`]: njoy_outram_park_fork::leapr::deck::LeaprDeck::parse
//! [`LeaprDeck::input_at_temperature`]: njoy_outram_park_fork::leapr::deck::LeaprDeck::input_at_temperature
//! [`FrequencyModel::start`]: njoy_outram_park_fork::leapr::frequency::FrequencyModel::start
//! [`phonon_expansion`]: njoy_outram_park_fork::leapr::continuous::phonon_expansion

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::continuous::phonon_expansion;
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::frequency::FrequencyModel;
use njoy_outram_park_fork::leapr::input::ElasticOption;
use njoy_outram_park_fork::leapr::vintage::PhysicalConstants;
use njoy_outram_park_fork::thermr::mf7::parse_mf7_at_temperature;

/// Default location of the ENDF/B-VIII.0 thermal-scattering sublibrary.
const DEFAULT_DIR: &str = "/home/teddy0/Documents/research/ENDF-B-VIII.0/thermal_scatt";
/// The LEAPR job that generated the graphite evaluation.
const DECK_FILE: &str = "tsl-crystalline-graphite.leapr";
/// The ENDF tape it generated.
const TAPE_FILE: &str = "tsl-crystalline-graphite.endf";
/// MAT number carried by both (the deck's card 4 says 30, despite the file name
/// and the MF=1/451 comment text both saying "crystalline graphite ... MAT 31").
const MAT: i32 = 30;

/// This crate's Boltzmann constant (CODATA2018), `common::phys::BK_EV_PER_K`.
const BK_CRATE: f64 = 8.617_333_262e-5;
/// The Boltzmann constant NJOY2016 carried until 23 Oct 2017, i.e. when the
/// SEP17 graphite evaluation was produced.
const BK_EVAL_ERA: f64 = 8.617_385e-5;

/// Resolve a data-file path (env override -> default), or `None` + a skip note.
fn data_path(file: &str) -> Option<std::path::PathBuf> {
    let dir = std::env::var("GRAPHITE_TSL_DIR").unwrap_or_else(|_| DEFAULT_DIR.to_string());
    let p = std::path::Path::new(&dir).join(file);
    if p.exists() {
        Some(p)
    } else {
        eprintln!(
            "SKIP leapr_graphite_deck_parity: {file} not found under {dir} (set GRAPHITE_TSL_DIR)"
        );
        None
    }
}

/// The registered constant set whose `bk` equals `bk`.
///
/// Panics on an unregistered value: the point of
/// [`PhysicalConstants`](njoy_outram_park_fork::leapr::vintage::PhysicalConstants)
/// is that the constant is a named, documented input, so a bare literal that
/// matches nothing is a mistake rather than a measurement.
fn constants_for(bk: f64) -> PhysicalConstants {
    PhysicalConstants::all()
        .into_iter()
        .find(|c| c.bk_ev_per_k() == bk)
        .unwrap_or_else(|| panic!("bk = {bk:e} is not a registered PhysicalConstants variant"))
}

/// Parse the deck, or `None` if the data is absent.
fn load_deck() -> Option<LeaprDeck> {
    let p = data_path(DECK_FILE)?;
    let text = std::fs::read_to_string(p).expect("deck is readable");
    Some(LeaprDeck::parse(&text).expect("graphite deck parses"))
}

/// One parity measurement over the shared alpha/beta grid.
struct Parity {
    /// Points compared (tape value above `1e-30`).
    n: usize,
    /// Largest relative deviation.
    max: f64,
    /// Root-mean-square relative deviation.
    rms: f64,
    /// Points the tape stores as exactly zero.
    zeros_tape: usize,
    /// Points this port produces as exactly zero.
    zeros_ours: usize,
    /// Points where exactly one of the two is zero.
    zero_mismatch: usize,
    /// `(max, rms, n)` restricted to tape values `>= 1e-9`, which `endout`
    /// stores with 7 significant figures.
    band7: (f64, f64, usize),
    /// `(max, rms, n)` restricted to tape values `< 1e-9`, stored with 6.
    band6: (f64, f64, usize),
}

/// Regenerate `S(alpha, beta)` at `temperature_k` using Boltzmann constant `bk`,
/// and compare against the tape's MF=7/MT=4 table at `temperature_k`.
///
/// Returns the parity statistics and the wall-clock time the generation took.
fn measure(deck: &LeaprDeck, temperature_k: f64, bk: f64) -> (Parity, std::time::Duration) {
    // The Boltzmann constant is an explicit input on the job
    // (`njoy_outram_park_fork::leapr::vintage`), so the historical `tev = bk * T`
    // is reproduced by naming the constant set rather than by the equivalent-
    // temperature trick this test used before that field existed.
    // `LeaprDeck::input_at_temperature` already selects the deck's own
    // EVAL-SEP17 vintage; this overrides it so both constants can be measured.
    let mut input = deck
        .input_at_temperature(0, temperature_k)
        .expect("temperature block 0 exists");
    input.constants = constants_for(bk);

    let t0 = std::time::Instant::now();
    let freq = FrequencyModel::start(
        &input.continuous.rho,
        input.continuous.delta_ev,
        input.tev(),
        input.continuous.tbeta,
    );
    let sab = phonon_expansion(&input, &freq);
    let elapsed = t0.elapsed();

    let tape = Tape::read(std::fs::File::open(data_path(TAPE_FILE).unwrap()).unwrap()).unwrap();
    let ii = parse_mf7_at_temperature(&tape, MAT, Some(temperature_k))
        .expect("MF=7 parses")
        .incoherent_inelastic
        .expect("graphite has MT=4");
    assert_eq!(ii.temperature_k, temperature_k, "tabulated temperature");
    assert_eq!(ii.beta.len(), deck.beta.len(), "beta grid length");

    let sc = input.scale();
    let (mut max, mut sumsq, mut n) = (0.0f64, 0.0f64, 0usize);
    let (mut zt, mut zo, mut zm) = (0usize, 0usize, 0usize);
    let (mut m7, mut s7, mut n7) = (0.0f64, 0.0f64, 0usize);
    let (mut m6, mut s6, mut n6) = (0.0f64, 0.0f64, 0usize);
    for k in 0..deck.beta.len() {
        let be = deck.beta[k] * sc;
        let fold = (-be / 2.0).exp();
        for j in 0..deck.alpha.len() {
            let ours = sab.get(k, j) * fold;
            let theirs = ii.s_tables[k].s[j];
            if theirs == 0.0 {
                zt += 1;
            }
            if ours == 0.0 {
                zo += 1;
            }
            if (theirs == 0.0) != (ours == 0.0) {
                zm += 1;
            }
            if theirs <= 1e-30 {
                continue;
            }
            let rel = (ours - theirs).abs() / theirs;
            max = max.max(rel);
            sumsq += rel * rel;
            n += 1;
            // endout stores >= 1e-9 with 7 significant figures, below with 6.
            if theirs >= 1e-9 {
                m7 = m7.max(rel);
                s7 += rel * rel;
                n7 += 1;
            } else {
                m6 = m6.max(rel);
                s6 += rel * rel;
                n6 += 1;
            }
        }
    }
    let p = Parity {
        n,
        max,
        rms: (sumsq / n as f64).sqrt(),
        zeros_tape: zt,
        zeros_ours: zo,
        zero_mismatch: zm,
        band7: (m7, (s7 / n7.max(1) as f64).sqrt(), n7),
        band6: (m6, (s6 / n6.max(1) as f64).sqrt(), n6),
    };
    (p, elapsed)
}

/// The 12 KB ENDF/B-VIII.0 graphite LEAPR deck parses in full, card for card.
///
/// **Methodology.** Read the distributed `.leapr` file and assert every card
/// against the deck text: the run controls, the ENDF output controls, the
/// principal-scatterer block, the grid sizes, the spectrum, and the ten-entry
/// temperature list with its nine negative "reuse" entries. **Pass criterion:**
/// exact equality on every field, an empty
/// [`LeaprDeck::unsupported_features`](njoy_outram_park_fork::leapr::deck::LeaprDeck::unsupported_features)
/// list, and no value hand-transcribed into this test that the parser did not
/// read from the file (grid contents are checked by their length, endpoints and
/// monotonicity, not by transcription).
///
/// **Result (2026-08-13):** all assertions hold. Nothing in the deck resisted
/// parsing; the 34 MF=1/451 comment lines round-trip too.
#[test]
fn graphite_deck_parses_in_full() {
    let Some(d) = load_deck() else { return };

    assert_eq!(d.nout, 24, "card 1 nout");
    assert_eq!(d.title, "graphite", "card 2 title");
    assert_eq!(d.ntempr(), 10, "card 3 ntempr");
    assert_eq!(d.iprint, 2, "card 3 iprint");
    assert_eq!(d.nphon, 100, "card 3 nphon");
    assert_eq!(d.mat, MAT, "card 4 mat");
    assert_eq!(d.za, 130.0, "card 4 za");
    // card 4 is `/`-terminated after za, so these must come from NJOY's defaults
    assert_eq!(d.isabt, 0, "card 4 isabt default");
    assert_eq!(d.ilog, 0, "card 4 ilog default");
    assert_eq!(d.smin, 1.0e-75, "card 4 smin default");
    assert_eq!(d.awr, 11.898, "card 5 awr");
    assert_eq!(d.spr, 4.73918, "card 5 spr");
    assert_eq!(d.npr, 1, "card 5 npr");
    assert_eq!(d.iel, ElasticOption::Graphite, "card 5 iel");
    assert_eq!(d.nsk, 0, "card 5 nsk default");
    assert_eq!(d.nss, 0, "card 6 nss");
    assert!(d.lat, "card 7 lat");
    assert_eq!(d.alpha.len(), 150, "card 7 nalpha");
    assert_eq!(d.beta.len(), 400, "card 9 nbeta");
    assert!(
        d.alpha.windows(2).all(|w| w[1] > w[0]),
        "alpha grid strictly increasing"
    );
    assert!(
        d.beta.windows(2).all(|w| w[1] > w[0]),
        "beta grid strictly increasing"
    );
    assert_eq!(d.beta[0], 0.0, "beta grid starts at 0");

    // Card 13 reads `0. 0. 1. 0./` — four values into a three-value read. If the
    // trailing `0.` leaked, `nd` would be misread and the oscillator path would
    // switch on silently. See `leapr::deck` for the record-terminator rules.
    let t0 = &d.temperatures[0];
    assert!(!t0.inherited, "296 K block is read, not inherited");
    assert_eq!(t0.continuous.delta_ev, 0.001, "card 11 delta");
    assert_eq!(t0.continuous.rho.len(), 201, "card 11 ni");
    assert_eq!(t0.continuous.rho[0], 0.0, "rho(0) = 0");
    assert_eq!(t0.continuous.twt, 0.0, "card 13 twt");
    assert_eq!(t0.continuous.c, 0.0, "card 13 c");
    assert_eq!(t0.continuous.tbeta, 1.0, "card 13 tbeta");
    assert!(t0.oscillators.is_empty(), "card 14 nd = 0");
    assert!(t0.pair_correlation.is_none(), "no cards 17-19");

    assert_eq!(
        d.temperatures_k(),
        vec![296.0, 400.0, 500.0, 600.0, 700.0, 800.0, 1000.0, 1200.0, 1600.0, 2000.0],
        "card 10 temperature list"
    );
    assert!(
        d.temperatures[1..].iter().all(|t| t.inherited),
        "temperatures 2..10 reuse the 296 K spectrum"
    );
    assert!(
        d.temperatures[1..]
            .iter()
            .all(|t| t.continuous == t0.continuous),
        "inherited blocks carry the same rho(E)"
    );

    assert!(
        d.unsupported_features().is_empty(),
        "deck uses only supported options: {:?}",
        d.unsupported_features()
    );
    assert_eq!(d.comments.len(), 34, "card 20 comment lines");
    assert!(
        d.comments[0].contains("Graphite") && d.comments[0].contains("Hawari"),
        "first comment line: {:?}",
        d.comments[0]
    );
}

/// Every LEAPR deck ENDF/B-VIII.0 ships parses, not just graphite's.
///
/// **Methodology.** Walk `*.leapr` in the thermal-scattering directory (33 files
/// in ENDF/B-VIII.0) and parse each one, asserting only structural invariants
/// that must hold for any valid deck: a non-empty alpha and beta grid, at least
/// one temperature, every temperature positive, and each block's `rho(E)`
/// holding at least two points with a positive `delta`. Between them these decks
/// exercise list-directed features graphite does not — `tsl-HinYH2.leapr` and
/// `tsl-YinYH2.leapr` use `85*0.00000E+00` repeat counts and pass the
/// undocumented `iel = -1`; several carry trailing prose after the `/`
/// terminator; the cold-moderator decks carry cards 17-19.
/// **Pass criterion:** every file parses and satisfies the invariants; any
/// failure is reported by name.
///
/// **Result (2026-08-13):** 33 of 33 decks parse. Two required parser work that
/// graphite alone would not have surfaced: `r*v` repeat counts, and `iel = -1`
/// (which NJOY's own card comments do not document — they list 0..=6 — but which
/// `endout` acts on at `leapr.f90:3053`). Nothing in any of the 33 resisted
/// parsing.
#[test]
fn every_endf_b_viii_leapr_deck_parses() {
    let dir = std::env::var("GRAPHITE_TSL_DIR").unwrap_or_else(|_| DEFAULT_DIR.to_string());
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("SKIP every_endf_b_viii_leapr_deck_parses: {dir} not readable");
        return;
    };
    let mut decks: Vec<std::path::PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "leapr"))
        .collect();
    decks.sort();
    if decks.is_empty() {
        eprintln!("SKIP every_endf_b_viii_leapr_deck_parses: no .leapr files under {dir}");
        return;
    }

    let mut failures: Vec<String> = Vec::new();
    for p in &decks {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let text = match std::fs::read_to_string(p) {
            Ok(t) => t,
            Err(e) => {
                failures.push(format!("{name}: unreadable: {e}"));
                continue;
            }
        };
        match LeaprDeck::parse(&text) {
            Err(e) => failures.push(format!("{name}: {e}")),
            Ok(d) => {
                let mut bad = Vec::new();
                if d.alpha.is_empty() {
                    bad.push("empty alpha grid");
                }
                if d.beta.is_empty() {
                    bad.push("empty beta grid");
                }
                if d.temperatures.is_empty() {
                    bad.push("no temperatures");
                }
                if d.temperatures.iter().any(|t| t.temperature_k <= 0.0) {
                    bad.push("non-positive temperature");
                }
                if d.temperatures
                    .iter()
                    .any(|t| t.continuous.rho.len() < 2 || t.continuous.delta_ev <= 0.0)
                {
                    bad.push("degenerate frequency spectrum");
                }
                if bad.is_empty() {
                    eprintln!(
                        "  {name}: {} alpha x {} beta, {} temps, iel={:?}, ncold={:?}, unsupported={:?}",
                        d.alpha.len(),
                        d.beta.len(),
                        d.ntempr(),
                        d.iel,
                        d.ncold,
                        d.unsupported_features()
                    );
                } else {
                    failures.push(format!("{name}: {}", bad.join(", ")));
                }
            }
        }
    }
    eprintln!("{} decks scanned, {} failures", decks.len(), failures.len());
    assert!(
        failures.is_empty(),
        "decks that did not parse: {failures:#?}"
    );
}

/// The regenerated `S(alpha, beta)` matches the official 296 K MF=7/MT=4 table.
///
/// **Methodology and pass criterion:** see the module documentation. In short —
/// generate from the deck over its own 150 x 400 grid at `nphon = 100`, apply
/// `endout`'s `S = ssm * exp(-beta_scaled/2)` conversion, and require max
/// relative deviation `< 1e-5` and RMS `< 2e-6` against the tape (and an
/// identical zero pattern) when run with the Boltzmann constant NJOY carried at
/// evaluation time. The CODATA2018 (as-shipped) figures are asserted separately
/// at their own, looser, honestly-stated level.
///
/// **Result (2026-08-13):** era-constant max 4.917e-6, RMS 6.390e-7 over 48,941
/// points; zero pattern identical (6,645 points, zero mismatches).
/// CODATA2018 max 3.711e-4, RMS 6.842e-5. Split by the tape's own storage
/// precision: 7-figure band max 4.948e-7 / RMS 1.355e-7 (37,950 points, against
/// a 5.0e-7 rounding ceiling), 6-figure band max 4.917e-6 / RMS 1.325e-6
/// (10,991 points, against a 5.0e-6 ceiling) — both at, and neither above,
/// their own ceiling.
#[test]
fn sab_parity_against_endf_tape_at_296k() {
    let Some(d) = load_deck() else { return };

    let (p, dt) = measure(&d, 296.0, BK_EVAL_ERA);
    eprintln!(
        "296 K, bk = {BK_EVAL_ERA:e}: n = {}, max = {:.3e}, rms = {:.3e}, \
         zeros tape/ours = {}/{} (mismatch {}), \
         7-fig band max {:.3e} rms {:.3e} over {}, \
         6-fig band max {:.3e} rms {:.3e} over {}, generated in {:.2} s",
        p.n,
        p.max,
        p.rms,
        p.zeros_tape,
        p.zeros_ours,
        p.zero_mismatch,
        p.band7.0,
        p.band7.1,
        p.band7.2,
        p.band6.0,
        p.band6.1,
        p.band6.2,
        dt.as_secs_f64()
    );

    assert_eq!(p.n, 48_941, "points compared");
    assert_eq!(
        p.zero_mismatch, 0,
        "zero pattern must match the tape exactly"
    );
    assert_eq!(p.zeros_tape, p.zeros_ours, "same number of stored zeros");
    assert!(p.max < 1.0e-5, "max relative deviation = {:.3e}", p.max);
    assert!(p.rms < 2.0e-6, "rms relative deviation = {:.3e}", p.rms);
    // The 7-significant-figure band must be the tighter of the two, which is
    // what distinguishes storage round-off from a physics discrepancy.
    assert!(
        p.band7.1 < p.band6.1,
        "7-fig rms {:.3e} should beat 6-fig rms {:.3e}",
        p.band7.1,
        p.band6.1
    );

    // The as-shipped CODATA2018 constant: still good, an order of magnitude
    // looser, and recorded rather than hidden.
    let (p18, _) = measure(&d, 296.0, BK_CRATE);
    eprintln!(
        "296 K, bk = {BK_CRATE:e} (crate default): max = {:.3e}, rms = {:.3e}",
        p18.max, p18.rms
    );
    assert!(p18.max < 1.0e-3, "CODATA2018 max = {:.3e}", p18.max);
    assert!(p18.rms < 2.0e-4, "CODATA2018 rms = {:.3e}", p18.rms);
    assert!(
        p18.max > p.max,
        "the era constant must be the better match ({:.3e} vs {:.3e})",
        p.max,
        p18.max
    );
}

/// The same parity holds at 1000 K, reached through the deck's negative-
/// temperature spectrum reuse.
///
/// **Methodology.** Identical to [`sab_parity_against_endf_tape_at_296k`] but at
/// the tabulated 1000 K point, whose deck block carries `inherited = true`. This
/// checks that `FrequencyModel::start` is correctly re-run per temperature
/// (`deltab`, `f0`, `tbar` and `T_1` all move with `tev`) rather than the 296 K
/// model being reused, and that the parity is not a single-temperature fluke.
/// **Pass criterion:** max relative deviation `< 1e-5`, RMS `< 2e-6`, zero
/// pattern identical.
///
/// **Result (2026-08-13):** max 4.838e-6, RMS 5.817e-7 over 52,378 points;
/// zero pattern identical (6,328 points, zero mismatches). The frequency model
/// does move with temperature as it must: `deltab` 0.039204 -> 0.011605,
/// Debye-Waller `lambda` 0.868056 -> 8.595487, `tbar` 2.385905 -> 1.172673
/// between 296 K and 1000 K.
#[test]
fn sab_parity_against_endf_tape_at_1000k() {
    let Some(d) = load_deck() else { return };
    assert!(
        d.temperatures[6].inherited && d.temperatures[6].temperature_k == 1000.0,
        "block 6 is the inherited 1000 K entry"
    );

    let (p, dt) = measure(&d, 1000.0, BK_EVAL_ERA);
    eprintln!(
        "1000 K, bk = {BK_EVAL_ERA:e}: n = {}, max = {:.3e}, rms = {:.3e}, \
         zeros tape/ours = {}/{} (mismatch {}), generated in {:.2} s",
        p.n,
        p.max,
        p.rms,
        p.zeros_tape,
        p.zeros_ours,
        p.zero_mismatch,
        dt.as_secs_f64()
    );

    assert_eq!(
        p.zero_mismatch, 0,
        "zero pattern must match the tape exactly"
    );
    assert!(p.max < 1.0e-5, "max relative deviation = {:.3e}", p.max);
    assert!(p.rms < 2.0e-6, "rms relative deviation = {:.3e}", p.rms);
}

/// The Debye-Waller conversion `W' = lambda / (awr * T * k_B)` reproduces the
/// tape's own temperature dependence.
///
/// **Why this is here.** `endout` consumes `dwpix` already divided by
/// `awr * T * k_B` (`leapr.f90:3035`), but `FrequencyModel` exposes only the raw
/// `lambda` (`f0`) and *no code path in this crate performs that division*. The
/// MT=4 parity above cannot catch a mistake in it, because the inelastic law
/// uses the raw `lambda`. This test closes that gap using the tape itself.
///
/// **Methodology.** MF=7/MT=2 stores the coherent-elastic `S(E, T)` as a
/// *cumulative* sum over Bragg edges, `S(E_i, T) = sum_{j <= i} f_j
/// exp(-4 W'(T) E_j)`. Differencing consecutive grid points isolates one edge,
/// `inc_i(T) = f_i exp(-4 W'(T) E_i)`, so for two temperatures the unknown
/// structure factor `f_i` cancels:
///
/// ```text
///   ln inc_i(T_a) - ln inc_i(T_b) = -4 E_i (W'(T_a) - W'(T_b))
/// ```
///
/// A least-squares fit of that difference against `-4 E_i` over every edge with
/// a usable increment therefore measures `W'(T_a) - W'(T_b)` from the tape
/// alone, with no lattice constants, no `coher` call and no assumption about
/// `f_i`. Compare against the prediction from this port's own `lambda(T)`:
/// `W'(T) = lambda(T) / (awr * T * k_B)`. **Pass criterion:** the fitted and
/// predicted `W'(2000 K) - W'(296 K)` agree to better than 1 %.
///
/// **Result (2026-08-13):** fitted `W'(2000 K) - W'(296 K) = 1.360726e1 /eV`
/// from 94 usable tape Bragg edges, against `1.367313e1 /eV` predicted — **0.48
/// % apart**. The conversion is therefore correct in shape as well as scale over
/// this range: `W'(296 K) = 2.860298 /eV` and `W'(2000 K) = 16.53343 /eV`.
///
/// Caveat on what this can and cannot show: only a *difference* of `W'` is
/// observable this way, because the structure factors `f_i` cancel only in a
/// ratio. An error common to every temperature would survive it. What it does
/// rule out is a wrong `awr`, a wrong `1/T`, a wrong `lambda(T)` trend, or a
/// missing/duplicated `k_B` — the plausible ways to get this two-line division
/// wrong.
///
/// **Secondary finding, recorded because it matters elsewhere.** At the *real*
/// `W'(296 K) = 2.86 /eV` the official tape retains only **221** of the **345**
/// Bragg edges `coher` generates up to 5 eV, with **no** trailing zero-increment
/// points. NJOY's `1/E` tail thinning (`leapr.f90:3210-3217`, `tol = 0.9e-7`)
/// therefore **does** fire for graphite, and the collapsed-tail path — where
/// every edge beyond `jmax` accumulates into the last retained point — **is**
/// exercised by this evaluation. Any judgement of that quirk as unreachable is
/// an artefact of a synthetic Debye-Waller value: `W'` around `8e-3 /eV` is some
/// 350x too small for graphite.
#[test]
fn debye_waller_conversion_matches_the_tape_temperature_trend() {
    let Some(d) = load_deck() else { return };
    let Some(tp) = data_path(TAPE_FILE) else {
        return;
    };
    let tape = Tape::read(std::fs::File::open(tp).unwrap()).unwrap();
    let ce = njoy_outram_park_fork::thermr::mf7::parse_mf7(&tape, MAT)
        .unwrap()
        .coherent_elastic
        .expect("graphite has coherent elastic");

    // Index the two temperatures in the tape's own S(E, T) table.
    let ia = ce
        .temperatures_k
        .iter()
        .position(|&t| t == 2000.0)
        .expect("2000 K tabulated");
    let ib = ce
        .temperatures_k
        .iter()
        .position(|&t| t == 296.0)
        .expect("296 K tabulated");

    // Least-squares slope of (ln inc_a - ln inc_b) against -4 E, through the
    // origin: the relation has no intercept.
    let (mut num, mut den, mut used) = (0.0f64, 0.0f64, 0usize);
    for i in 1..ce.bragg_energies_ev.len() {
        let e = ce.bragg_energies_ev[i];
        let inc_a = ce.s_tables[ia][i] - ce.s_tables[ia][i - 1];
        let inc_b = ce.s_tables[ib][i] - ce.s_tables[ib][i - 1];
        // Only edges that actually open, and with enough magnitude that the
        // tape's 7-figure storage of a *difference* is still meaningful.
        if inc_a <= 0.0 || inc_b <= 0.0 {
            continue;
        }
        if inc_b < 1e-6 * ce.s_tables[ib][i] {
            continue;
        }
        let x = -4.0 * e;
        let y = inc_a.ln() - inc_b.ln();
        num += x * y;
        den += x * x;
        used += 1;
    }
    assert!(used > 20, "only {used} usable Bragg edges for the fit");
    let fitted = num / den;
    let n_edges = ce.bragg_energies_ev.len();
    let e_top = *ce.bragg_energies_ev.last().unwrap();
    let s_top = *ce.s_tables[ib].last().unwrap();
    let tail_flat = ce.s_tables[ib]
        .windows(2)
        .rev()
        .take_while(|w| w[1] == w[0])
        .count();
    // How many edges did `coher` generate before NJOY's 1/E thinning? Comparing
    // that with the count the tape retained says whether the thinning fired.
    let generated = njoy_outram_park_fork::leapr::coher::coher(
        njoy_outram_park_fork::leapr::coher::CoherentLattice::Graphite,
        d.npr as usize,
        5.0,
    )
    .edges
    .len();
    eprintln!(
        "tape MT=2: {n_edges} Bragg grid points retained of {generated} generated by coher, \
         E_max = {e_top:.6} eV, S(E_max, 296 K) = {s_top:.6}, \
         trailing points with zero increment = {tail_flat}"
    );

    // Prediction from this port: W'(T) = lambda(T) / (awr * T * k_B).
    let wprime = |t_k: f64| -> f64 {
        let mut inp = d.input_at_temperature(0, t_k).unwrap();
        inp.constants = constants_for(BK_EVAL_ERA);
        let f = FrequencyModel::start(
            &inp.continuous.rho,
            inp.continuous.delta_ev,
            inp.tev(),
            inp.continuous.tbeta,
        );
        f.f0 / (d.awr * t_k * BK_EVAL_ERA)
    };
    let predicted = wprime(2000.0) - wprime(296.0);
    let rel = (fitted - predicted).abs() / predicted.abs();
    eprintln!(
        "Debye-Waller: W'(2000 K) - W'(296 K) = {fitted:.6e} /eV fitted from {used} tape Bragg \
         edges vs {predicted:.6e} /eV predicted from lambda/(awr*T*k_B) -> {:.3} % apart \
         [W'(296) = {:.6e}, W'(2000) = {:.6e} /eV]",
        100.0 * rel,
        wprime(296.0),
        wprime(2000.0)
    );
    assert!(
        rel < 0.01,
        "fitted {fitted:.6e} vs predicted {predicted:.6e} /eV ({:.2} % apart)",
        100.0 * rel
    );
    // Pin the secondary finding: at graphite's real W' the tape is thinned, so
    // NJOY's collapsed-tail path is reachable and must stay modelled.
    assert!(
        n_edges < generated,
        "NJOY's 1/E thinning retained {n_edges} of {generated} edges — if this ever \
         becomes an equality, the collapsed-tail quirk stopped being exercised"
    );
    assert_eq!(
        tail_flat, 0,
        "a thinned tape should have no trailing zero-increment points"
    );
}

/// Cost of regenerating one temperature, in release mode.
///
/// **Methodology.** Time `FrequencyModel::start` + `phonon_expansion` for each
/// of the deck's ten temperatures on the full 150 x 400 grid at `nphon = 100`
/// (no I/O, no tape parsing inside the timed region). This decides whether deck
/// regeneration is a build-time step or can be done at run time.
/// **Pass criterion:** none imposed on the absolute time beyond a loose
/// sanity ceiling — the number itself is the deliverable.
///
/// **Result (2026-08-13, release, 12-core workstation).** The measurement is
/// **contention-limited, not CPU-limited**, and the honest statement of it is a
/// bound rather than a point value: this box was shared with other agents
/// throughout, and the same single-threaded work measured
///
/// - **1.749-1.823 s** per temperature (17.89 s for all ten) at load average
///   ~7, and
/// - 3.3-10.7 s per temperature (65.7 s for all ten) at load average ~19.6.
///
/// The workload is single-threaded and allocation-light, so those differ only
/// by CPU share. Contention can only inflate a timing, so **~1.75 s per
/// temperature is an upper bound on the uncontended cost**; anything above it is
/// scheduling noise, and a quiet machine would need re-measuring to sharpen it.
/// The cost is flat across 296-2000 K (`deltab` shrinks with temperature but the
/// work is set by `nphon` and `ni`, not by `tev`) and is dominated by `convol`,
/// whose `T_n` array grows to `nphon * (ni - 1) + 1 = 20,001` points — roughly
/// `ni * sum_n(n * (ni - 1))` ~ 2e8 multiply-adds. Memory is negligible: the
/// `ssm` array is 60,000 doubles = 480 kB and the largest `T_n` is 160 kB.
///
/// **Interpretation:** order-seconds per temperature is a *build-time* or
/// first-use-cached cost, not a per-query one, and that conclusion is robust to
/// the factor-of-four contention spread above. Shipping the 12 KB deck in place
/// of the 8.7 MB tape is a 701x storage saving for well under a minute of
/// one-off compute to recover all ten tabulated temperatures, or a couple of
/// seconds to produce a single untabulated one such as HTR-10's 393 K or 523 K.
#[test]
fn regeneration_cost_per_temperature() {
    let Some(d) = load_deck() else { return };

    let mut total = 0.0;
    for (i, t) in d.temperatures.iter().enumerate() {
        let input = d.input_at(i).unwrap();
        let t0 = std::time::Instant::now();
        let freq = FrequencyModel::start(
            &input.continuous.rho,
            input.continuous.delta_ev,
            input.tev(),
            input.continuous.tbeta,
        );
        let sab = phonon_expansion(&input, &freq);
        let dt = t0.elapsed().as_secs_f64();
        total += dt;
        // Touch the result so the optimiser cannot elide the work.
        assert!(sab.get(0, 0).is_finite());
        eprintln!(
            "T = {:>6.1} K: deltab = {:.6}, lambda = {:.6}, tbar = {:.6}, {:.3} s",
            t.temperature_k, freq.deltab, freq.f0, freq.tbar, dt
        );
    }
    eprintln!(
        "total for {} temperatures: {:.2} s ({:.2} s each)",
        d.ntempr(),
        total,
        total / d.ntempr() as f64
    );
    assert!(
        total < 600.0,
        "regeneration of {} temperatures took {total:.1} s",
        d.ntempr()
    );
}
