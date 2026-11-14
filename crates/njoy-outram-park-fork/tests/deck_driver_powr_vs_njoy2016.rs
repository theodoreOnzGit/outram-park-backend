// SPDX-License-Identifier: AGPL-3.0-only
//! **POWR from a deck, whole decks against NJOY2016.80** (gh:#1373).
//!
//! # Methodology
//!
//! No upstream test deck runs POWR, so the decks are our own (the constants
//! below). Each runs NJOY2016's deck-11 chain on the Pu-238 ENDF/B-IV tape
//! `t404` (`tests/resources`, unit 20): MODER, RECONR, BROADR (300, 900 and
//! 2100 K), UNRESR (7 sigma zeroes), for the thermal deck THERMR (free gas,
//! MT221), and GROUPR to a **coded** GENDF on unit 26; then POWR to unit 27.
//! The whole deck is run by the NJOY2016.80 binary (`NJOY2016_ROOT/build/njoy`)
//! and by [`run_deck`], and every listed unit is compared **as strings, line by
//! line, trailing blanks included** (`mergt` pads the fast library to 80
//! columns). Pass criterion: zero differing lines on every unit, more than 20
//! lines on POWR's unit, and the deck runs to its `stop`. A case skips with a
//! message when the NJOY clone is absent.
//!
//! - [`powr_fast_lib1_whole_deck`]: `lib = 1`, GROUPR `ign = 6` (68-group
//!   GAM-II), elastic P0/P1, inelastic 51-59 and 91, fission with its
//!   spectrum, f-factors over 7 sigma zeroes and 3 temperatures.
//! - [`powr_thermal_lib2_whole_deck`]: `lib = 2`, GROUPR `ign = 8`
//!   (35-group), three material cards at 300, 900 and 2100 K, `itrc = 0` and
//!   `1`, `sigma s` given and defaulted.
//! - [`powr_fast_n2n_whole_deck`]: `lib = 1` with an MF=6 (n,2n) matrix,
//!   which upstream reads through its label-390 quirk.
//! - [`powr_thermal_text_label_stops_as_upstream`]: `lib = 2` on a GENDF
//!   whose label is text, where upstream stops.
//! - [`powr_cpm_lib3_is_refused`]: `lib = 3` stops at POWR.
//!
//! Ablations (in the first two tests): the same comparison with the fast
//! library's trailing blanks trimmed, and with our deck run at `izref = 2`
//! (fast) or `itrc` flipped (thermal) against NJOY's run of the original deck;
//! each must report differing lines, so the gate can fail.
//!
//! # Prediction (posted on gh:#1373 before the first run)
//!
//! `lib = 1` and `lib = 2` identical on every line; NJOY2016 stops on the
//! (n,2n) deck and the port refuses it by name; `lib = 3` refused by name.
//! **Wrong on the (n,2n) deck**: NJOY2016 runs it to its end, because
//! `contio` reads a data line with `6e11.0` and `nint`s it; the refusal was
//! removed and the case is compared like the others.
//!
//! # Results (2026-11-14, NJOY2016.80 `ac5adf5`, gfortran 13.3, release)
//!
//! Every unit identical, as strings, on the first run of each deck that NJOY
//! runs to its end (GENDF lines / POWR library lines, words):
//!
//! | test | GENDF | library |
//! |---|---|---|
//! | fast (Pu-238, ign 6) | 4 535 | 577 (3 325 words) |
//! | (n,2n) | 4 559 | 600 (3 459) |
//! | delayed (Th-232) | 4 362 | 725 (4 179) |
//! | collapsed fast (185 groups) | 11 983 | 559 (3 205) |
//! | thermal (free gas, ign 8) | 2 557 | 413 (2 585) |
//! | collapsed thermal | 12 632 | 687 (4 229) |
//! | graphite (MT229/230) | 716 | 120 (755) |
//!
//! Ablations: 577 differing lines (trimmed), 46 (`izref = 2`), 404 (`itrc`).
//! Wrong predictions (recorded on gh:#1373): the (n,2n) deck (NJOY runs it),
//! and the thermal deck's first version, which NJOY stops on because `therm`
//! reads GROUPR's text title with `contio`; the port now stops there too.
//! Record: `verification_and_validation/deck_driver/README.md` §39.

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

mod common;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

fn differing(a: &str, b: &str) -> Vec<usize> {
    let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect()
}

/// The chain every deck starts with: Pu-238 from `t404` to the PENDF on
/// unit -24 (broadened, unresolved self-shielding tables at 7 sigma zeroes).
const CHAIN: &str = "moder
 20 -21/
reconr
 -21 -22/
 'pendf tape for pu-238 from endf/b-iv tape 404'/
 1050 3/
 .005/
 '94-pu-238 from endf/b tape t404'/
 'processed by the njoy nuclear data processing system'/
 'see original endf/b-iv tape for details of evaluation'/
 0/
broadr
 -21 -22 -23/
 1050 3 0 1 0 /
 .005/
 300. 900. 2100./
 0/
unresr
 -21 -23 -24/
 1050 3 7 1/
 300 900 2100/
 1.e10 1.e5 1.e4 1000. 100. 10. 1/
 0/
";

/// GROUPR `ign = 6`, iwt = 5, P1, 3 temperatures, 7 sigma zeroes; `{n2n}` is
/// an optional extra MF=6 request at 300 K.
fn fast_deck(n2n: &str, izref: i32) -> String {
    fast_deck_with(n2n, izref, "", "6", "")
}

/// [`fast_deck`] with `extra3` (more MF=3/MF=5 requests at 300 K, after MT102)
/// and GROUPR's `ign` and group cards (`groups`, empty for a built-in set).
fn fast_deck_with(n2n: &str, izref: i32, extra3: &str, ign: &str, groups: &str) -> String {
    format!(
        "{CHAIN}groupr
 -21 -24 0 26/
 1050 {ign} 0 5 1 3 7 1/
 '94-pu-238'/
 300. 900. 2100./
 1.e10 1.e5 1.e4 1000. 100. 10. 1/
{groups} 3 1 'total'/
 3 2 'elastic'/
 3 18 'fission'/
 3 102 'capture'/
{extra3} 6 2 'elastic'/
{n2n} 6 18 'fission'/
 6 51 'discrete inelastic'/
 6 -59 'continued'/
 6 91 'continuum inelastic'/
 0/
 3 1 'total'/
 3 18 'fission'/
 3 102 'capture'/
 0/
 3 1 'total'/
 3 18 'fission'/
 3 102 'capture'/
 0/
 0/
powr
 26 27/
 1 1 0/
 1050 300. 1 7 {izref}/
 'pu-238 test'/
 'pu-238 fission spectrum'/
 0/
stop
"
    )
}

/// THERMR free gas (deck 11's card), GROUPR `ign = 8` (35 groups), POWR
/// `lib = 2` at the three temperatures; `itrc` of the first card is `{itrc}`.
fn thermal_deck(itrc: i32, title: &str) -> String {
    thermal_deck_with(itrc, title, "8", "")
}

/// [`thermal_deck`] with GROUPR's `ign` and group cards.
fn thermal_deck_with(itrc: i32, title: &str, ign: &str, groups: &str) -> String {
    format!(
        "{CHAIN}thermr
 0 -24 -25/
 0 1050 8 3 1 0 0 1 221 0/
 300. 900. 2100./
 .05 4.2/
groupr
 -21 -25 0 26/
 1050 {ign} 0 5 1 3 7 1/
 '{title}'/
 300. 900. 2100./
 1.e10 1.e5 1.e4 1000. 100. 10. 1/
{groups} 3 1 'total'/
 3 2 'elastic'/
 3 18 'fission'/
 3 102 'capture'/
 3 221 'free gas'/
 6 221 'free gas'/
 0/
 3 1 'total'/
 3 18 'fission'/
 3 102 'capture'/
 6 221 'free gas'/
 0/
 3 1 'total'/
 3 102 'capture'/
 6 221 'free gas'/
 0/
 0/
powr
 26 27/
 2/
 1050 300 'pu238t300'/
 {itrc} 221 0/
 .1 .2 .3 2.9 3.2e-11 1.0e-11 .5 0/
 1050 900 'pu238 t900ab'/
 1 221 0/
 .1 .2 .3 2.9 3.2e-11 1.0e-11 .5 10./
 1050 2100 'pu-238 hot'/
 1 221/
 /
 0/
stop
"
    )
}

/// `t404` (Pu-238 ENDF/B-IV) on unit 20.
const T404: &[(&str, u32)] = &[("t404", 20)];

/// GROUPR cards 6a/6b for `ign = 1`: 185 groups, logarithmic from 1e-5 eV
/// to 20 MeV (only the count matters to POWR's collapse, `icgrp`).
fn groups_185() -> String {
    let mut s = String::from(" 185/\n");
    for k in 0..=185 {
        let e = 1.0e-5f64 * (2.0e12f64).powf(f64::from(k) / 185.0);
        s.push_str(&format!(" {e:.6e}"));
        if k % 6 == 5 || k == 185 {
            s.push('\n');
        }
    }
    s.pop();
    s.push_str("/\n");
    s
}

/// The thermal check on another evaluation and law: C-nat ENDF/B-V (`t511`,
/// deck 01) with the graphite S(a,b) of `t322` (THERMR MT229 inelastic and
/// MT230 coherent elastic, deck 01's cards), GROUPR `ign = 8` at 296 K, POWR
/// `lib = 2` with `mti = 229`, `mtc = 230`.
const GRAPHITE_DECK: &str = "moder
 20 -21/
reconr
 -21 -22/
 'pendf tape for c-nat from endf/b tape 511'/
 1306 3/
 .005/
 '6-c-nat from tape 511'/
 'processed by the njoy nuclear data processing system'/
 'see original endf/b-v tape for details of evaluation'/
 0/
broadr
 -21 -22 -23/
 1306 1/
 .005/
 296./
 0/
thermr
 26 -23 -24/
 1065 1306 8 1 2 1 0 1 229 2/
 296./
 .05 1.2/
groupr
 -21 -24 0 27/
 1306 8 0 5 1 1 1 1/
 ' '/
 296
 1.e10/
 3 1 'total'/
 3 2 'elastic'/
 3 102 'n,g'/
 3 229 'graphite inelastic'/
 3 230 'graphite elastic'/
 6 229 'graphite inelastic'/
 6 230 'graphite elastic'/
 0/
 0/
powr
 27 28/
 2 1/
 1306 296 'graphite'/
 1 229 230/
 .158 .716 .056 0 0 0 0 4.7/
 0/
stop
";

/// Run `deck` in NJOY2016.80 (when `njoy`) and here, in `tmp/<tag>/{njoy,ours}`
/// with each `(resource, unit)` mounted. Returns `(ours, theirs, njoy ok, our run)`.
fn run_both(
    tag: &str,
    deck: &str,
    resources: &[(&str, u32)],
) -> Option<(PathBuf, PathBuf, bool, Result<njoy_outram_park_fork::deck::DeckRun, njoy_outram_park_fork::NjoyError>)> {
    let root = njoy_root();
    let bin = root.join("build/njoy");
    if !bin.exists() {
        eprintln!("skip: {} not present (set NJOY2016_ROOT)", bin.display());
        return None;
    }
    let tmp = std::env::temp_dir().join(format!("njoy_deck_powr_{tag}_{}", std::process::id()));
    let (ours, theirs) = (tmp.join("ours"), tmp.join("njoy"));
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        for (r, u) in resources {
            let res = root.join("tests/resources").join(r);
            if !res.exists() {
                eprintln!("skip: {} not present", res.display());
                return None;
            }
            std::fs::copy(&res, d.join(format!("tape{u}"))).unwrap();
        }
    }
    std::fs::write(theirs.join("deck"), deck).unwrap();
    let (ok, date) = common::run_njoy_dated(&bin, &theirs);
    let run = run_deck(deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date) });
    Some((ours, theirs, ok, run))
}

fn read(d: &Path, u: u32) -> String {
    std::fs::read_to_string(d.join(format!("tape{u}"))).unwrap_or_default()
}

fn cleanup(ours: &Path) {
    if std::env::var_os("DECK_KEEP").is_none() {
        let _ = std::fs::remove_dir_all(ours.parent().unwrap());
    }
}

/// Compare `units`; returns `(unit, differing, lines)` of the failures.
fn compare_units(tag: &str, ours: &Path, theirs: &Path, units: &[u32]) -> Vec<(u32, usize, usize)> {
    let mut bad = Vec::new();
    for &u in units {
        let (a, b) = (read(ours, u), read(theirs, u));
        let d = differing(&a, &b);
        let n = b.lines().count();
        let words: usize = b.split_whitespace().count();
        println!("{tag} unit {u}: {} differing lines of {n} ({words} words)", d.len());
        for &i in d.iter().take(4) {
            println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, a.lines().nth(i), b.lines().nth(i));
        }
        if !d.is_empty() || n < 20 {
            bad.push((u, d.len(), n));
        }
    }
    bad
}

/// **`lib = 1` (fast), whole deck**: GROUPR's GENDF (unit 26) and POWR's
/// library (unit 27). Ablations: unit 27 with its trailing blanks trimmed, and
/// our deck at `izref = 2` against NJOY's `izref = 1`.
#[test]
fn powr_fast_lib1_whole_deck() {
    let deck = fast_deck("", 1);
    let Some((ours, theirs, ok, run)) = run_both("fast", &deck, T404) else { return };
    assert!(ok, "NJOY2016 failed on the fast deck");
    let run = run.unwrap();
    assert_eq!(run.blocked, None, "the fast deck must run to its end");
    let bad = compare_units("fast", &ours, &theirs, &[26, 27]);
    // ablation 1: the 80-column padding is part of what is compared
    let trimmed: String = read(&ours, 27).lines().map(|l| format!("{}\n", l.trim_end())).collect();
    let d1 = differing(&trimmed, &read(&theirs, 27)).len();
    println!("fast ablation (trailing blanks trimmed): {d1} differing lines");
    assert!(d1 > 0, "trimming the padding must be seen");
    // ablation 2: the reference sigma zero of the elastic matrix
    let ab = run_deck(
        &fast_deck("", 2),
        &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some("11/14/26".into()) },
    )
    .unwrap();
    let lib = ab.units.text(27).unwrap_or_default().to_string();
    let d2 = differing(&lib, &read(&theirs, 27)).len();
    println!("fast ablation (izref = 2 against NJOY's izref = 1): {d2} differing lines");
    assert!(d2 > 0, "a different reference sigma zero must be seen");
    cleanup(&ours);
    assert!(bad.is_empty(), "fast: (unit, differing, lines) {bad:?}");
}

/// **`lib = 2` (thermal), whole deck**: GENDF unit 26 and the library on 27.
/// Ablation: our deck with the first card's `itrc = 1` against NJOY's `0`.
#[test]
fn powr_thermal_lib2_whole_deck() {
    let deck = thermal_deck(0, " ");
    let Some((ours, theirs, ok, run)) = run_both("thermal", &deck, T404) else { return };
    assert!(ok, "NJOY2016 failed on the thermal deck");
    let run = run.unwrap();
    assert_eq!(run.blocked, None, "the thermal deck must run to its end");
    let bad = compare_units("thermal", &ours, &theirs, &[26, 27]);
    let ab = run_deck(
        &thermal_deck(1, " "),
        &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some("11/14/26".into()) },
    )
    .unwrap();
    let lib = ab.units.text(27).unwrap_or_default().to_string();
    let d = differing(&lib, &read(&theirs, 27)).len();
    println!("thermal ablation (itrc = 1 against NJOY's itrc = 0): {d} differing lines");
    assert!(d > 0, "the transport-correction option must be seen");
    cleanup(&ours);
    assert!(bad.is_empty(), "thermal: (unit, differing, lines) {bad:?}");
}

/// **`lib = 1` with an MF=6 (n,2n) matrix**: `gamll`'s label 390 returns to
/// 250 in the middle of the section and reads the next records' lines as CONT
/// records. Upstream runs to its end; units 26 and 27 compared as above.
#[test]
fn powr_fast_n2n_whole_deck() {
    let deck = fast_deck(" 6 16 'n2n'/\n", 1);
    let Some((ours, theirs, ok, run)) = run_both("n2n", &deck, T404) else { return };
    assert!(ok, "NJOY2016 failed on the (n,2n) deck");
    assert_eq!(run.unwrap().blocked, None, "the (n,2n) deck must run to its end");
    let bad = compare_units("n2n", &ours, &theirs, &[26, 27]);
    cleanup(&ours);
    assert!(bad.is_empty(), "n2n: (unit, differing, lines) {bad:?}");
}

/// **`lib = 2` on a GENDF whose label is GROUPR's title `94-pu-238`**:
/// `therm` reads the label with `contio` (`6e11.0`), so NJOY2016.80 stops
/// ("Bad value during floating point read") and writes nothing on unit 27;
/// the port stops with an error saying why.
#[test]
fn powr_thermal_text_label_stops_as_upstream() {
    let deck = thermal_deck(0, "94-pu-238");
    let Some((ours, theirs, ok, run)) = run_both("thermal_label", &deck, T404) else { return };
    let log = std::fs::read_to_string(theirs.join("log")).unwrap_or_default();
    println!("njoy ok = {ok}; unit 27 lines {}", read(&theirs, 27).lines().count());
    assert!(!ok && log.contains("Bad value during floating point read"), "NJOY2016 must stop on the label");
    let e = match run {
        Ok(_) => panic!("the port must stop too"),
        Err(e) => e,
    };
    println!("ours: {e}");
    assert!(e.to_string().contains("label"), "{e}");
    cleanup(&ours);
}

/// **`lib = 1` with delayed neutrons**: the fast deck on Th-232 (MAT 1296 of
/// `t404`, ENDF/B-IV) with GROUPR's MF=3
/// MT455 (delayed nu) and MF=5 MT455 (6 delayed spectra), which `gamll`
/// counts into `nfs` and `gamxs` normalises (label 330 and the `dnu` block),
/// so the library carries 6 delayed chi blocks.
#[test]
fn powr_fast_delayed_whole_deck() {
    // Pu-238 (MAT 1050) on t404 has no delayed data; Th-232 (MAT 1296) on
    // the same tape has MF=1 and MF=5 MT455 with 6 groups.
    let deck = fast_deck_with("", 1, " 3 455 'nu'/\n 5 455 'delayed chi'/\n", "6", "")
        .replace("1050", "1296")
        .replace(" 6 -59 'continued'/", " 6 -58 'continued'/");
    let Some((ours, theirs, ok, run)) = run_both("delayed", &deck, T404) else { return };
    assert!(ok, "NJOY2016 failed on the delayed deck");
    assert_eq!(run.unwrap().blocked, None, "the delayed deck must run to its end");
    let bad = compare_units("delayed", &ours, &theirs, &[26, 27]);
    cleanup(&ours);
    assert!(bad.is_empty(), "delayed: (unit, differing, lines) {bad:?}");
}

/// **`lib = 1` and `lib = 2` collapsed from 185 groups**: GROUPR `ign = 1`
/// with a 185-group structure, so `icgrp` maps through `igf` (fast) and `igt`
/// (thermal), as upstream intends for its 185-group master library.
#[test]
fn powr_collapsed_from_185_groups_whole_deck() {
    let g = groups_185();
    for (tag, deck) in [
        ("collapse_fast", fast_deck_with("", 1, "", "1", &g)),
        ("collapse_thermal", thermal_deck_with(1, " ", "1", &g)),
    ] {
        let Some((ours, theirs, ok, run)) = run_both(tag, &deck, T404) else { return };
        assert!(ok, "NJOY2016 failed on {tag}");
        assert_eq!(run.unwrap().blocked, None, "{tag} must run to its end");
        let bad = compare_units(tag, &ours, &theirs, &[26, 27]);
        cleanup(&ours);
        assert!(bad.is_empty(), "{tag}: (unit, differing, lines) {bad:?}");
    }
}

/// **`lib = 2` on graphite** ([`GRAPHITE_DECK`]): a thermal elastic MT
/// (`mtc = 230`) beside the inelastic one, one temperature, `itrc = 1`.
#[test]
fn powr_thermal_graphite_whole_deck() {
    let Some((ours, theirs, ok, run)) = run_both("graphite", GRAPHITE_DECK, &[("t511", 20), ("t322", 26)]) else {
        return;
    };
    assert!(ok, "NJOY2016 failed on the graphite deck");
    assert_eq!(run.unwrap().blocked, None, "the graphite deck must run to its end");
    let bad = compare_units("graphite", &ours, &theirs, &[27, 28]);
    cleanup(&ours);
    assert!(bad.is_empty(), "graphite: (unit, differing, lines) {bad:?}");
}

/// **`lib = 3` (EPRI-CPM)**: not translated; the deck stops at POWR with the
/// refusal named.
#[test]
fn powr_cpm_lib3_is_refused() {
    let deck = "powr\n 26 27/\n 3 0 1/\n 1 1 1 0/\nstop\n";
    let run = run_deck(deck, &DeckOptions { workdir: None, parse_only: false, run_date: None }).unwrap();
    let blocked = run.blocked.expect("lib = 3 must be refused");
    assert_eq!(blocked.0, "powr");
    assert!(blocked.1.contains("lib = 3"), "{blocked:?}");
    // a negative GENDF unit is refused too
    let deck = "powr\n -26 27/\n 1/\n 0/\nstop\n";
    let run = run_deck(deck, &DeckOptions { workdir: None, parse_only: false, run_date: None }).unwrap();
    assert!(run.blocked.expect("binary refused").1.contains("blocked-binary"));
}
