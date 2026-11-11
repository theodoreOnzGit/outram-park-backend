// SPDX-License-Identifier: AGPL-3.0-only
//! **NJOY input-deck driver, tape against tape** (gh:#1369 part 2, differential
//! #1366).
//!
//! Upstream's own regression decks (`tests/NN/input`) are run by the NJOY2016.80
//! binary and by [`run_deck`], cut before the first module that has no tape
//! driver here and with MODER conversions appended so each binary unit is
//! written as text. **Every line** of each output tape is compared: the tape
//! label, MF=1/MT=451 with its description and dictionary, MF=2, MF=3 and the
//! MF=12/13 sections RECONR carries. This replaces part 1's section-level
//! comparison of MF=3 only.
//!
//! The NJOY clone (binary `build/njoy`, `tests/resources/`, `tests/NN/input`)
//! is located by `NJOY2016_ROOT`, else `upstream_source/NJOY2016`; a case
//! skips with a message when it is absent.
//!
//! # Predictions (written before the runs)
//!
//! - Test 01 (C-nat, ENDF/B-V, no resonances): every line of all three tapes
//!   identical. MF=3 already was; the new content is the dictionary, MF=2 and
//!   MF=12/13.
//! - Test 07 (U-235, ENDF/B-V, SLBW resolved + `LFW=1` unresolved): identical
//!   everywhere except where RECONR's unresolved `csunr1` enters (port wiring
//!   uses UNRESR's kernel, `reconr/urr.rs`): MF=3 MT=18/19/102 (and MT=301
//!   through HEATR) inside the unresolved range and the MF=2/MT=152 table.
//!
//! # Results (2026-11-11, rustc per rust-toolchain.toml, gfortran 13.3)
//!
//! Test 01: **0 differing lines** of 2 848 / 2 734 / 3 358 (units 28, 29, 30).
//! First run (before the fixes below): MF=1 text and dictionary absent, MF=2
//! absent, MF=12/13 absent, `L1/L2` of every MF=3 head.
//!
//! Tests 07, 10, 19: see each test's doc for the measured results; the first run of 07
//! differed in 396 lines of 17 897 (SLBW elastic, `csslbw` rounds the running
//! elastic sum to 8 figures after every l-state and the port did not: **port
//! defect, fixed**), `lrp` (3 with an unresolved range) and BROADR's update
//! of the MT=152 temperature. After those three, 56 lines remain, all in the
//! unresolved range (see the test).

use std::path::{Path, PathBuf};
use std::process::Command;

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// The upstream deck of `case`, cut before module `cut` (exclusive, `None`: whole
/// deck minus nothing), with `tail` cards appended and `stop`.
fn deck_text(root: &Path, case: &str, cut: Option<&str>, tail: &str) -> Option<String> {
    let input = std::fs::read_to_string(root.join(format!("tests/{case}/input"))).ok()?;
    let mut out = String::new();
    for line in input.lines() {
        if line.trim() == "stop" {
            break;
        }
        if cut.is_some_and(|c| line.trim() == c) {
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(tail);
    out.push_str("stop\n");
    Some(out)
}

/// Run `deck` in NJOY and in this crate on `resource` mounted as unit 20.
/// Returns `(ours, njoy)` work directories, or `None` (skip).
fn run_pair(root: &Path, tag: &str, resource: &str, deck: &str) -> Option<(PathBuf, PathBuf)> {
    let bin = root.join("build/njoy");
    let res = root.join("tests/resources").join(resource);
    for p in [&bin, &res] {
        if !p.exists() {
            eprintln!("skip: {} not present (set NJOY2016_ROOT)", p.display());
            return None;
        }
    }
    let tmp = std::env::temp_dir().join(format!("njoy_deck_{tag}_{}", std::process::id()));
    let (ours, theirs) = (tmp.join("ours"), tmp.join("njoy"));
    std::fs::create_dir_all(&ours).unwrap();
    std::fs::create_dir_all(&theirs).unwrap();
    for d in [&ours, &theirs] {
        std::fs::copy(&res, d.join("tape20")).unwrap();
    }
    std::fs::write(theirs.join("deck"), deck).unwrap();
    let st = Command::new("sh")
        .arg("-c")
        .arg(format!("{} < deck > log 2>&1", bin.display()))
        .current_dir(&theirs)
        .status()
        .unwrap();
    assert!(st.success(), "NJOY2016 binary failed on {tag}");
    let run = run_deck(deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false }).unwrap();
    assert_eq!(run.blocked, None, "{tag}: the cut deck must run to its end");
    Some((ours, theirs))
}

/// Differing lines of two text tapes: `(line number, "MAT MF MT" of the NJOY line)`.
/// A line-count difference is reported as a line past the end of the shorter.
fn differing_lines(ours: &Path, njoy: &Path) -> Vec<(usize, String)> {
    let a = std::fs::read_to_string(ours).unwrap();
    let b = std::fs::read_to_string(njoy).unwrap();
    let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let mut out = Vec::new();
    for i in 0..al.len().max(bl.len()) {
        let (x, y) = (al.get(i).copied().unwrap_or(""), bl.get(i).copied().unwrap_or(""));
        if x != y {
            let key = if y.len() >= 75 { y[66..75].to_string() } else { "past end".to_string() };
            out.push((i + 1, key));
        }
    }
    out
}

/// **Test 01 through MODER, RECONR, BROADR, HEATR: every line identical.**
///
/// Deck: upstream `tests/01/input` up to THERMR, MODER conversions inserted
/// after RECONR (unit 28) and at the end (29 after BROADR, 30 after HEATR).
/// Pass criterion: zero differing lines on units 28, 29, 30.
#[test]
fn test_01_front_half_tapes_are_identical_line_for_line() {
    let root = njoy_root();
    let Some(base) = deck_text(&root, "01", Some("thermr"), "moder\n -23 29/\nmoder\n -22 30/\n") else { return };
    let deck = base.replacen("broadr\n", "moder\n -22 28/\nbroadr\n", 1);
    let Some((ours, njoy)) = run_pair(&root, "t01", "t511", &deck) else { return };
    for u in [28, 29, 30] {
        let d = differing_lines(&ours.join(format!("tape{u}")), &njoy.join(format!("tape{u}")));
        println!("test 01 unit {u}: {} differing lines", d.len());
        assert!(d.is_empty(), "unit {u}: first differences {:?}", &d[..d.len().min(5)]);
    }
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// Differing lines whose `MAT MF MT` key (columns 67-75 of the NJOY line) is
/// not in `allowed`.
fn unexpected(d: &[(usize, String)], allowed: &[&str]) -> Vec<(usize, String)> {
    d.iter().filter(|(_, k)| !allowed.contains(&k.as_str())).cloned().collect()
}

/// **Test 07 (U-235 ENDF/B-V, SLBW + `LFW=1` unresolved) through MODER, RECONR,
/// BROADR, HEATR, MODER**, against NJOY2016.80, line by line.
///
/// Deck: upstream `tests/07/input` up to GROUPR (its own `moder -24 28` ends
/// the cut deck). Pass criterion: the only differing lines are those RECONR's
/// unresolved evaluation reaches (gh:#1448: `csunr1` not ported, the port uses
/// UNRESR's kernel): MF=3 MT=18, 19, 102 and 301 and MF=2/MT=152. **Measured
/// 2026-11-11: 56 of 17 897 lines differ, all in those five sections**; the
/// first run, before the `csslbw` fix (gh:#1447), differed in 396 lines
/// including every elastic and total section of the resolved range.
#[test]
fn test_07_pendf_differs_only_where_csunr1_enters() {
    let root = njoy_root();
    let Some(deck) = deck_text(&root, "07", Some("groupr"), "") else { return };
    let Some((ours, njoy)) = run_pair(&root, "t07", "t511", &deck) else { return };
    let d = differing_lines(&ours.join("tape28"), &njoy.join("tape28"));
    println!("test 07 unit 28: {} differing lines", d.len());
    let bad = unexpected(&d, &["1395 3 18", "1395 3 19", "1395 3102", "1395 3301", "1395 2152"]);
    assert!(bad.is_empty(), "outside the unresolved range: {:?}", &bad[..bad.len().min(5)]);
    assert!(d.len() <= 56, "more differing lines than the recorded 56: {}", d.len());
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// Both ACE tables of one deck run, parsed.
fn ace_pair(ours: &Path, njoy: &Path, unit: i32) -> njoy_outram_park_fork::acer::compare::AceComparison {
    use njoy_outram_park_fork::acer::{compare::compare, read::parse_type1};
    let read = |d: &Path| parse_type1(&std::fs::read_to_string(d.join(format!("tape{unit}"))).unwrap()).unwrap();
    compare(&read(ours), &read(njoy))
}

/// **Test 10 (Pu-238 ENDF/B-IV): the whole deck, `moder; reconr; broadr` (three
/// temperatures, bootstrap) `; unresr; purr; acer`**, ACE unit 26 against
/// NJOY2016.80 word for word, and the PENDF after BROADR line for line.
///
/// Prediction (before running): the ACE agrees except where the ENDF-IV
/// formats bite (MF=4 `LVT = 1`, an MT=18 angular table); the PENDF differs
/// only in MF=2/MT=152, RECONR's unresolved table (gh:#1448).
///
/// **Result (2026-11-11): the ACE table is identical in every word**
/// (NXS, JXS and all 38 713 XSS words, PURR's probability bands included at
/// `nladr = 4`), after three port defects found by this deck and fixed
/// (gh:#1447: `LVT` skip, MT=18 angular, MT=4 start index; BROADR bootstrap
/// rounding). The PENDF after BROADR: 6 differing lines, all MF=2/MT=152.
#[test]
fn test_10_ace_is_identical_and_pendf_differs_only_in_mt152() {
    let root = njoy_root();
    let Some(full) = deck_text(&root, "10", None, "") else { return };
    let Some((ours, njoy)) = run_pair(&root, "t10", "t404", &full) else { return };
    let c = ace_pair(&ours, &njoy, 26);
    for l in c.report_lines() {
        println!("{l}");
    }
    assert!(c.identical(), "test 10 ACE differs from NJOY2016");

    let Some(head) = deck_text(&root, "10", Some("unresr"), "moder\n -23 29/\n") else { return };
    let Some((o2, n2)) = run_pair(&root, "t10b", "t404", &head) else { return };
    let d = differing_lines(&o2.join("tape29"), &n2.join("tape29"));
    println!("test 10 unit 29 (3 temperatures): {} differing lines", d.len());
    let bad = unexpected(&d, &["1050 2152"]);
    assert!(bad.is_empty(), "{:?}", &bad[..bad.len().min(5)]);
    for dir in [&ours, &o2] {
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}

/// **Test 19 (Pu-241 ENDF/B-VI.3): `moder; reconr; broadr; unresr; heatr; purr;
/// acer`**, ACE unit 27 against NJOY2016.80, and the PENDF after BROADR.
///
/// Result (2026-11-11): every block of the ACE table is identical **except
/// LUNR (PURR's probability bands): 760 of 4 604 words differ** (gh:#1450, not
/// understood: the first incident energy's total-band column is already
/// different). NXS, JXS and all 63 136 other words are identical. The PENDF
/// after BROADR differs in 6 lines, MF=2/MT=152 and the MT=1 point at 2.5e4 eV, the
/// unresolved boundary (gh:#1448). The test pins exactly
/// that: it fails if any other block differs, and if LUNR starts to agree it
/// fails so this record gets updated.
#[test]
fn test_19_ace_differs_only_in_lunr_and_pendf_only_in_mt152() {
    let root = njoy_root();
    let Some(full) = deck_text(&root, "19", None, "") else { return };
    let Some((ours, njoy)) = run_pair(&root, "t19", "e6pu241c", &full) else { return };
    let c = ace_pair(&ours, &njoy, 27);
    for l in c.report_lines() {
        println!("{l}");
    }
    assert!(c.nxs.differences.is_empty() && c.jxs.differences.is_empty());
    let differing: Vec<&str> = c.blocks.iter().filter(|b| !b.verdict.agrees()).map(|b| b.name).collect();
    assert_eq!(differing, vec!["LUNR"], "recorded: only LUNR differs");

    let Some(head) = deck_text(&root, "19", Some("unresr"), "moder\n -23 29/\n") else { return };
    let Some((o2, n2)) = run_pair(&root, "t19b", "e6pu241c", &head) else { return };
    let d = differing_lines(&o2.join("tape29"), &n2.join("tape29"));
    println!("test 19 unit 29: {} differing lines", d.len());
    let bad = unexpected(&d, &["9443 2152", "9443 3  1"]);
    assert!(bad.is_empty(), "{:?}", &bad[..bad.len().min(5)]);
    for dir in [&ours, &o2] {
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}
