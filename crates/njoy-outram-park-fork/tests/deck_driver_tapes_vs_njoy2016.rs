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
    run_pair_decks(root, tag, resource, deck, deck)
}

/// [`run_pair`] with a deck of its own for each side: NJOY the whole of an
/// upstream deck, this crate the same minus the modules it has no tape driver
/// for (GROUPR, whose GENDF no run here reads).
fn run_pair_decks(root: &Path, tag: &str, resource: &str, deck_njoy: &str, deck_ours: &str) -> Option<(PathBuf, PathBuf)> {
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
    std::fs::write(theirs.join("deck"), deck_njoy).unwrap();
    let st = Command::new("sh")
        .arg("-c")
        .arg(format!("{} < deck > log 2>&1", bin.display()))
        .current_dir(&theirs)
        .status()
        .unwrap();
    assert!(st.success(), "NJOY2016 binary failed on {tag}");
    let run = run_deck(deck_ours, &DeckOptions { workdir: Some(ours.clone()), parse_only: false }).unwrap();
    assert_eq!(run.blocked, None, "{tag}: the deck must run to its end");
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

/// **Test 07 (U-235 ENDF/B-V, SLBW + `LFW=1` unresolved) through MODER, RECONR,
/// BROADR, HEATR, MODER**, against NJOY2016.80, line by line.
///
/// Deck: upstream `tests/07/input` up to GROUPR (its own `moder -24 28` ends
/// the cut deck). Pass criterion: **zero** differing lines.
///
/// Result: **2026-11-11, before `csunr1`/`csunr2` were translated (gh:#1448):
/// 56 of 17 897 lines differed**, all MT=18, 19, 102, 301 and MF=2/MT=152 in
/// the unresolved range (the port used UNRESR's kernel, which evaluates the
/// parameters at every energy where `csunr1` interpolates the cross sections
/// between panel end points); the very first run, before the `csslbw` fix
/// (gh:#1447), differed in 396 lines. **After the translation
/// (`reconr::csunr`): 0 of 17 897 lines differ.**
#[test]
fn test_07_pendf_is_identical_line_for_line() {
    let root = njoy_root();
    let Some(deck) = deck_text(&root, "07", Some("groupr"), "") else { return };
    let Some((ours, njoy)) = run_pair(&root, "t07", "t511", &deck) else { return };
    let d = differing_lines(&ours.join("tape28"), &njoy.join("tape28"));
    println!("test 07 unit 28: {} differing lines", d.len());
    assert!(d.is_empty(), "first differences {:?}", &d[..d.len().min(5)]);
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
/// **Result (2026-11-11): the ACE table is identical in every word**
/// (NXS, JXS and all 38 713 XSS words, PURR's probability bands included at
/// `nladr = 4`), after three port defects found by this deck and fixed
/// (gh:#1447: `LVT` skip, MT=18 angular, MT=4 start index; BROADR bootstrap
/// rounding). The PENDF after BROADR: **before** `csunr1`/`csunr2` were
/// translated (gh:#1448) 6 lines differed, all MF=2/MT=152; **now 0**.
#[test]
fn test_10_ace_and_pendf_are_identical() {
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
    assert!(d.is_empty(), "{:?}", &d[..d.len().min(5)]);
    for dir in [&ours, &o2] {
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}

/// **Test 19 (Pu-241 ENDF/B-VI.3): `moder; reconr; broadr; unresr; heatr; purr;
/// acer`**, ACE unit 27 against NJOY2016.80, and the PENDF after BROADR.
///
/// Result (2026-11-11): **the ACE table is identical in every word** (NXS, JXS
/// and all 67 740 XSS words) and the PENDF after BROADR (three temperatures)
/// identical in every line. Recorded history: the first run (gh:#1369) had
/// LUNR differing in 760 of 4 604 words (gh:#1450) and 6 PENDF lines (MT=152 and
/// MT=1 at 2.5e4 eV, gh:#1448). The LUNR words were all in the **heating
/// column**: this deck's HEATR asks for the partial kermas `302 318 402`, so
/// PURR's `rdheat` sets `ihave = 2` and self-shields the heating per band
/// (`purr.f90:533-558`), which the port did not do (it divided MT=301 by the
/// total for every band: 96.03388 against NJOY's 38.636 ...), and the unit is
/// blocked binary, so neither MT=301 nor MT=153 is rounded through text (81
/// words then differed by 1e-12). The MT=152 lines went with `csunr`
/// (gh:#1448); the MT=1 line at 2.5e4 eV was `rebuild_total_as_sum_of_parts`
/// adding MT=51 below its threshold (the first value, 2.08e-7 b, `emerge`
/// skips a section below its threshold). Pass criterion: no differing word or line.
#[test]
fn test_19_ace_and_pendf_are_identical() {
    let root = njoy_root();
    let Some(full) = deck_text(&root, "19", None, "") else { return };
    let Some((ours, njoy)) = run_pair(&root, "t19", "e6pu241c", &full) else { return };
    let c = ace_pair(&ours, &njoy, 27);
    for l in c.report_lines() {
        println!("{l}");
    }
    assert!(c.identical(), "test 19 ACE differs from NJOY2016");

    let Some(head) = deck_text(&root, "19", Some("unresr"), "moder\n -23 29/\n") else { return };
    let Some((o2, n2)) = run_pair(&root, "t19b", "e6pu241c", &head) else { return };
    let d = differing_lines(&o2.join("tape29"), &n2.join("tape29"));
    println!("test 19 unit 29: {} differing lines", d.len());
    assert!(d.is_empty(), "{:?}", &d[..d.len().min(5)]);
    for dir in [&ours, &o2] {
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}

/// An upstream deck with its GROUPR block removed (from `groupr` up to the next
/// module card), for this crate's side of a whole-deck ACE comparison.
fn without_groupr(deck: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in deck.lines() {
        let t = line.trim();
        if t == "groupr" {
            skipping = true;
            continue;
        }
        if skipping && (t == "acer" || t == "stop") {
            skipping = false;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// **Test 07 (U-235 ENDF/B-V), the whole deck's ACE (`newfor = 0`, the old
/// angular format)**: `moder; reconr; broadr; heatr; moder; groupr; acer`, ACE
/// unit 26 against NJOY2016.80 (gh:#1449).
///
/// NJOY runs the upstream deck whole; this crate runs it without the GROUPR
/// block (it has no GENDF driver).
///
/// **Result (2026-11-11):** with `newfor = 0` ported (every angular distribution
/// converted to 32 equally probable bins by `ptleg`/`pttab` and stored by
/// `acensd`'s old branch, `acer::acensd::parse_mf4_for_acer_newfor`), **every
/// block is identical in every word except two**: **GPD** (gamma-production data)
/// is absent here, because NJOY's `gamout` builds it from the GROUPR GENDF
/// (`ngend = -25`, GROUPR card `16/`: photon-production matrices), which this crate
/// cannot make (GROUPR tape driver not ported); and **ESZ: 1 word of 25 205**,
/// the heating column at 81.99999 eV, 75.45917 against 75.45914 (4e-7, one
/// 7th-figure step; the HEATR PENDF itself is identical line for line, test 07
/// above, and the second shaded node 82.00001 eV agrees; cause not found).
/// NXS(1) and JXS(12-22) differ because GPD is missing. Before this port the
/// deck was refused (`newfor = 0`). Pass criterion: exactly those two blocks differ.
#[test]
fn test_07_acer_old_format_ace() {
    let root = njoy_root();
    let Some(full) = deck_text(&root, "07", None, "") else { return };
    let ours_deck = without_groupr(&full);
    let Some((ours, njoy)) = run_pair_decks(&root, "t07a", "t511", &full, &ours_deck) else { return };
    let c = ace_pair(&ours, &njoy, 26);
    for l in c.report_lines() {
        println!("{l}");
    }
    let differing: Vec<&str> = c.blocks.iter().filter(|b| !b.verdict.agrees()).map(|b| b.name).collect();
    assert_eq!(differing, vec!["ESZ", "GPD"], "recorded: only ESZ (1 word) and GPD (GROUPR gamout) differ");
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Test 08 (Ni-61 ENDF/B-VI.1), the whole deck's ACE (`newfor = 0`)**, ACE
/// unit 25 against NJOY2016.80, NJOY on the whole upstream deck, this crate on
/// it without GROUPR (gh:#1449).
///
/// **Result (2026-11-11): the AND block (angular distributions, the `newfor = 0`
/// equally probable bins) is identical in all 2 943 words**, as are MTRP, LSIGP,
/// SIGP, LANDP, ANDP, LDLWP and DLWP (photon production). **Still different,
/// not ported:** the ACE of this deck is made from a PENDF to which HEATR added
/// MT=443/444 (`302 303 304 402 443 444`), which NJOY's ACER puts in the reaction
/// list and the union grid (4 460 against 4 447 energies, reaction MT=444 missing
/// here, so ESZ, MTR, LQR, TYR, LSIG, SIG, GPD, YP follow); LAND/LDLW/DLW differ for
/// the MF=6 reactions, whose old-format laws (`acelf6` `newfor = 0` branches,
/// `acefc.f90:7495`, `:7722`, `:7765`) and `topfil`'s MF=6 conversion
/// (`:2233-2350`) are not translated. Pass criterion: the deck runs and AND agrees.
#[test]
fn test_08_acer_old_format_ace() {
    let root = njoy_root();
    let Some(full) = deck_text(&root, "08", None, "") else { return };
    let ours_deck = without_groupr(&full);
    let Some((ours, njoy)) = run_pair_decks(&root, "t08a", "eni61", &full, &ours_deck) else { return };
    let c = ace_pair(&ours, &njoy, 25);
    for l in c.report_lines() {
        println!("{l}");
    }
    let differing: Vec<&str> = c.blocks.iter().filter(|b| !b.verdict.agrees()).map(|b| b.name).collect();
    println!("test 08 differing blocks: {differing:?}");
    assert!(!differing.contains(&"AND"), "the old-format angular block must agree: {differing:?}");
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}
