// SPDX-License-Identifier: AGPL-3.0-only
//! **THERMR's second SCT atom (`gatef2`) on an ENDF-4/5 tape, against NJOY2016.80**
//! (gh:#1375).
//!
//! # Methodology
//!
//! For an ENDF-4/5 tape THERMR keys a mixed moderator's second short-collision-time
//! atom on `matde` alone (`thermr.f90:218-238`): 1099 (BeO) and 1095 (benzene) get
//! built-in `sz2`, `az2`, and `gatef2`'s `T_eff2` (`:422`), which `sig` label 170
//! adds as a second term (`:2588-2594`). No BeO or benzene ENDF-4/5 tape is held,
//! here or in NJOY2016's `tests/resources`. So deck `G2` (ours) is upstream deck 01
//! up to GROUPR (MODER, RECONR, BROADR, HEATR, the free-gas THERMR, then THERMR on
//! `t322`'s ENDF/B-III graphite), with `t322` relabelled in the test from MAT 1065
//! to 1099 (columns 67-70 of every MAT-1065 line) and the second THERMR's `matde`
//! set to 1099, then `moder -23 31/`. Graphite's `S(alpha,beta)` with BeO's
//! effective temperatures and oxygen's SCT term has no physical meaning: this is a
//! code-to-code device only.
//!
//! NJOY2016.80 and [`run_deck`] run the same deck. Pass criterion: unit 31
//! identical line for line, compared as strings, and the deck runs to its `stop`.
//!
//! # Prediction (posted on gh:#1375 before the first run)
//!
//! Unit 31 identical; with the second term ablated, MF=3/MT=229 and MF=6/MT=229
//! differ (if `calcem` reaches label 170 below 1.2 eV); NJOY's `G2` unit 31
//! differs from deck 01's.
//!
//! # Results (2026-11-14, NJOY2016.80, release)
//!
//! - Unit 31 identical, 32 897 of 32 897 lines (19 635 of them MF=3/6 MT=229), on
//!   the first run.
//! - Control: NJOY's own unit 31 for `G2` differs from deck 01's (MAT 1065) in
//!   11 222 lines, so `matde = 1099` reaches the output.
//! - Ablation, the second SCT term dropped (`secondary_sct = None`, the port before
//!   gh:#1375): 2 063 lines differ, from MF=3/MT=229 at 0.553 eV on, and the
//!   MF=1 dictionary's MT=229 line count.
//! - Ablation, `gatef2`'s table bypassed (`teff2 = T`): 2 092 lines differ.

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

mod common;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// `t322` with every MAT-1065 line relabelled 1099 (columns 67-70).
fn relabel_1065_to_1099(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        if line.len() >= 70 && &line[66..70] == "1065" {
            out.push_str(&line[..66]);
            out.push_str("1099");
            out.push_str(&line[70..]);
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Deck 01 up to GROUPR, the second THERMR with `matde = matde`, and a MODER
/// of its output to unit 31.
fn deck(root: &Path, matde: &str) -> Option<String> {
    let input = std::fs::read_to_string(root.join("tests/01/input")).ok()?;
    let mut out = String::new();
    for line in input.lines() {
        if line.trim() == "groupr" {
            break;
        }
        out.push_str(&line.replace(" 1065 1306 8 1 2 1 0 1 229 2/", &format!(" {matde} 1306 8 1 2 1 0 1 229 2/")));
        out.push('\n');
    }
    out.push_str("moder\n -23 31/\nstop\n");
    Some(out)
}

fn lines(p: &Path) -> Vec<String> {
    std::fs::read_to_string(p).unwrap_or_default().lines().map(str::to_string).collect()
}

fn differing(a: &[String], b: &[String]) -> Vec<usize> {
    (0..a.len().max(b.len())).filter(|&i| a.get(i) != b.get(i)).collect()
}

/// **`G2`: deck 01's graphite THERMR as MAT 1099, so `gatef2` and the second SCT term run.**
#[test]
fn g2_thermr_endf3_second_sct_atom_gatef2() {
    let root = njoy_root();
    let bin = root.join("build/njoy");
    let (t511, t322) = (root.join("tests/resources/t511"), root.join("tests/resources/t322"));
    for p in [&bin, &t511, &t322] {
        if !p.exists() {
            eprintln!("skip: {} not present (set NJOY2016_ROOT)", p.display());
            return;
        }
    }
    let Some(g2) = deck(&root, "1099") else { return };
    assert!(g2.contains(" 1099 1306 8 1 2 1 0 1 229 2/"), "the deck edit must apply");
    let tmp = std::env::temp_dir().join(format!("njoy_deck_gatef2_{}", std::process::id()));
    let (ours, theirs, control) = (tmp.join("ours"), tmp.join("njoy"), tmp.join("njoy01"));
    let relabelled = relabel_1065_to_1099(&std::fs::read_to_string(&t322).unwrap());
    for d in [&ours, &theirs, &control] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::copy(&t511, d.join("tape20")).unwrap();
    }
    std::fs::write(ours.join("tape26"), &relabelled).unwrap();
    std::fs::write(theirs.join("tape26"), &relabelled).unwrap();
    std::fs::copy(&t322, control.join("tape26")).unwrap();
    std::fs::write(theirs.join("deck"), &g2).unwrap();
    std::fs::write(control.join("deck"), deck(&root, "1065").unwrap()).unwrap();
    let (ok, date) = common::run_njoy_dated(&bin, &theirs);
    assert!(ok, "NJOY2016 binary failed on G2");
    let (ok, _) = common::run_njoy_dated(&bin, &control);
    assert!(ok, "NJOY2016 binary failed on the deck-01 control");
    let run = run_deck(&g2, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date) }).unwrap();
    assert_eq!(run.blocked, None, "G2 must run to its end");
    let (a, b, c) = (lines(&ours.join("tape31")), lines(&theirs.join("tape31")), lines(&control.join("tape31")));
    let diff = differing(&a, &b);
    let n229 = b.iter().filter(|l| l.len() >= 75 && (&l[66..75] == "1306 3229" || &l[66..75] == "1306 6229")).count();
    let ctrl = differing(&b, &c).len();
    println!("G2 unit 31: {} differing lines of {} ({n229} in MF=3/6 MT=229); NJOY G2 against deck 01: {ctrl} lines differ", diff.len(), b.len());
    for &i in diff.iter().take(6) {
        println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, a.get(i), b.get(i));
    }
    if std::env::var_os("KEEP_DECK_DIRS").is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    assert!(n229 > 100, "NJOY's unit 31 must carry MT=229 ({n229})");
    assert!(ctrl > 0, "control: matde 1099 must reach NJOY's output");
    assert!(diff.is_empty(), "G2: {} of {} lines differ", diff.len(), b.len());
}
