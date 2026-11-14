// SPDX-License-Identifier: AGPL-3.0-only
//! **GROUPR's weight functions `iwt = 1, 7, 8, 10, 11, 12` and `-3`, whole decks
//! against NJOY2016.80** (gh:#1370).
//!
//! # Methodology
//!
//! No upstream deck uses these weights: the 87 decks use `iwt` 2 to 6 and 9 only.
//! So seven decks of our own are run by the NJOY2016.80 binary
//! (`NJOY2016_ROOT/build/njoy`) and by [`run_deck`] on the same tape, H-2
//! ENDF/B-VIII.0 (`tests/resources/n-001_H_002-ENDF8.0.endf`, MAT 128). Each deck
//! runs MODER, then RECONR (`err = 0.001`), then BROADR to 293.6 K, then GROUPR.
//! GROUPR uses LANL 30 groups (`ign = 3`), `lord = 1`, 293.6 K, `sigz = 1e10`, and
//! the reactions `3/` (the automatic MF=3 list) and `6 2/`. The GENDF is written
//! to the text unit 24. Only `iwt`, and its card 8a or 8b, changes between the
//! decks:
//!
//! - `W1`: `iwt = 1`, with card 8b a two-region TAB1 (INT 5 to point 2, then
//!   INT 2), four points;
//! - `W7`, `W8`, `W10`, `W11`, `W12`: the built-in weights. `7`, `10` and `12`
//!   are temperature-dependent and read `temp(jtemp)`;
//! - `WM3`: `iwt = -3` with `nsigz = 1`. Upstream calls `genflx` only when
//!   `nsigz > 1` (`groupr.f90:539`), so `getflx` takes `getwtf` with `|iwt|`
//!   (`:6505-6512`).
//!
//! Pass criterion: unit 24 is identical line for line, compared as strings, and
//! the deck runs to its `stop`. A case skips with a message when the NJOY clone
//! or the tape is absent.
//!
//! # Prediction (posted on gh:#1370 before the first run)
//!
//! Every line of unit 24 identical in all seven decks.
//!
//! One more deck, `MF2` (Pu-240, below), gates `mfchk`, the card-9 switch of
//! `mfd = 16` to 17 for an MT in MF=13.
//!
//! # Results (2026-11-13, NJOY2016.80 `ac5adf5`, release)
//!
//! - All seven weight decks: unit 24 identical, 321 of 321 lines each. Control:
//!   NJOY's GENDFs for these decks differ from each other (3 090 bytes between
//!   `iwt = 7` and `8`, 59 between 11 and 12).
//! - `MF2`: identical, 170 of 170 lines. Ablation: with `mfchk` returning its
//!   input, 96 of 170 lines differ (MF=16 MT=3 is written from MF=12 instead of
//!   MF=13), so the deck does exercise it.
//!
//! Record: deck-driver README §28; predictions and results on gh:#1370.

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

mod common;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// The line numbers (0-based) where `a` and `b` differ, as strings.
fn differing(a: &str, b: &str) -> Vec<usize> {
    let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect()
}

/// The H-2 deck with GROUPR's `iwt`, the cards after its card 7 (`weight`, empty
/// for the built-in weights) and `nsigz = 1`.
fn deck(iwt: i32, weight: &str) -> String {
    format!(
        "moder\n20 -21/\nreconr\n-21 -22/\n'h2'/\n128/\n0.001/\n0/\n\
         broadr\n-21 -22 -23/\n128 1/\n0.001/\n293.6/\n0/\n\
         groupr\n-21 -23 0 24/\n128 3 0 {iwt} 1 1 1 1/\n'h2 weights'/\n293.6/\n1.0e10/\n\
         {weight}3/\n6 2/\n0/\n0/\nstop\n"
    )
}

/// Run `deck` in NJOY2016.80 and here on H-2 and compare unit 24 line for line.
/// `None`: skipped.
fn compare(tag: &str, deck: &str) -> Option<()> {
    compare_on(tag, deck, "n-001_H_002-ENDF8.0.endf")
}

/// [`compare`] on the resource `tape` (mounted as unit 20).
fn compare_on(tag: &str, deck: &str, tape: &str) -> Option<()> {
    let root = njoy_root();
    let bin = root.join("build/njoy");
    let res = root.join("tests/resources").join(tape);
    if !bin.exists() || !res.exists() {
        eprintln!("skip: {} or {} not present (set NJOY2016_ROOT)", bin.display(), res.display());
        return None;
    }
    let tmp = std::env::temp_dir().join(format!("njoy_deck_gw_{tag}_{}", std::process::id()));
    let (ours, theirs) = (tmp.join("ours"), tmp.join("njoy"));
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::copy(&res, d.join("tape20")).unwrap();
    }
    std::fs::write(theirs.join("deck"), deck).unwrap();
    let (ok, date) = common::run_njoy_dated(&bin, &theirs); // gh:#1549
    assert!(ok, "NJOY2016 binary failed on {tag}");
    let run = run_deck(deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date.clone()) }).unwrap();
    assert_eq!(run.blocked, None, "{tag} must run to its end");
    let read = |d: &Path| std::fs::read_to_string(d.join("tape24")).unwrap_or_default();
    let (a, b) = (read(&ours), read(&theirs));
    let d = differing(&a, &b);
    println!("{tag} unit 24: {} differing lines of {}", d.len(), b.lines().count());
    for &i in d.iter().take(6) {
        println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, a.lines().nth(i), b.lines().nth(i));
    }
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(!b.is_empty(), "{tag}: NJOY2016 wrote no unit 24");
    assert!(d.is_empty(), "{tag}: unit 24 differs from NJOY2016.80");
    Some(())
}

/// `W1`: `iwt = 1`, card 8b a TAB1 `0 0 0 0 nr=2 np=4 / 2 5 4 2 / (E, w) x 4`.
#[test]
fn w1_groupr_read_in_tab1_weight() {
    let card8b = "0. 0. 0 0 2 4\n2 5 4 2\n1.e-5 1.e5 1.0 1.0 1.e5 1.e-5 2.e7 3.e-8/\n";
    let _ = compare("w1", &deck(1, card8b));
}

/// `W7`: `iwt = 7`, (thermal) -- (1/E) -- (fission + fusion), thermal part at 293.6 K.
#[test]
fn w7_groupr_temperature_dependent_thermal_fission_fusion() {
    let _ = compare("w7", &deck(7, ""));
}

/// `W8`: `iwt = 8`, thermal -- 1/E -- fast reactor -- fission + fusion (`w8`).
#[test]
fn w8_groupr_fast_reactor_weight() {
    let _ = compare("w8", &deck(8, ""));
}

/// `W10`: `iwt = 10`, CLAW with a thermal Maxwellian below `0.15 T/300` eV.
#[test]
fn w10_groupr_claw_temperature_dependent() {
    let _ = compare("w10", &deck(10, ""));
}

/// `W11`: `iwt = 11`, VITAMIN-E.
#[test]
fn w11_groupr_vitamin_e() {
    let _ = compare("w11", &deck(11, ""));
}

/// `W12`: `iwt = 12`, VITAMIN-E with its thermal part at 293.6 K.
#[test]
fn w12_groupr_vitamin_e_temperature_dependent() {
    let _ = compare("w12", &deck(12, ""));
}

/// `WM3`: `iwt = -3` (card 8a read) with `nsigz = 1`: no flux calculation upstream.
#[test]
fn wm3_groupr_flux_calculator_card_with_one_sigma_zero() {
    let _ = compare("wm3", &deck(-3, "1.e4 11.29 0 -30/\n"));
}

/// `MF2`: GROUPR's `mfchk` (`groupr.f90:1526-1573`, gh:#1370) on Pu-240
/// ENDF/B-VIII.0 (MAT 9440), whose only MF=13 section is MT=3 (`NK = 1`). Card 9
/// asks `16 3` and `16 102`: upstream switches `16 3` to `mfd = 17` (MF=13) and
/// keeps `16 102`. RECONR at `err = 0.01`, GROUPR at 0 K, LANL-30, LANL 12 photon
/// groups (`igg = 3`), `iwt = 3`, `lord = 0`.
///
/// (`MF1`, on B-10, was dropped before any comparison: its MF=13 MT=4 and 103 have
/// 16 and 9 photons, which `getgyl` refuses by name.)
#[test]
fn mf2_groupr_mfchk_switches_16_to_17_on_mf13() {
    let deck = "moder\n20 -21/\nreconr\n-21 -22/\n'pu240'/\n9440/\n0.01/\n0/\n\
                groupr\n-21 -22 0 24/\n9440 3 3 3 0 1 1 1/\n'pu240 mfchk'/\n0.0/\n1.0e10/\n\
                3 1/\n16 3/\n16 102/\n0/\n0/\nstop\n";
    let _ = compare_on("mf2", deck, "n-094_Pu_240-ENDF8.0.endf");
}
