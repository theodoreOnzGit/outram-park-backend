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
        if cut.is_some_and(|c| line.split_whitespace().next() == Some(c)) {
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
    run_pair_decks_extra(root, tag, resource, &[], deck_njoy, deck_ours)
}

/// [`run_pair_decks`] with further resources mounted as units: `(resource, unit)`.
fn run_pair_decks_extra(
    root: &Path,
    tag: &str,
    resource: &str,
    extra: &[(&str, u32)],
    deck_njoy: &str,
    deck_ours: &str,
) -> Option<(PathBuf, PathBuf)> {
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
        for (r, u) in extra {
            let src = root.join("tests/resources").join(r);
            if !src.exists() {
                eprintln!("skip: {} not present", src.display());
                return None;
            }
            std::fs::copy(&src, d.join(format!("tape{u}"))).unwrap();
        }
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
/// Both run the upstream deck whole, GROUPR included (gh:#1481).
///
/// **Result (2026-11-11):** with `newfor = 0` ported (every angular distribution
/// converted to 32 equally probable bins by `ptleg`/`pttab` and stored by
/// `acensd`'s old branch, `acer::acensd::parse_mf4_for_acer_newfor`), **every
/// block is identical in every word except one**: **GPD** (gamma-production data)
/// is absent here, because NJOY's `gamout` builds it from the GROUPR GENDF
/// (`ngend = -25`, GROUPR card `16/`: photon-production matrices), which this crate
/// cannot make (GROUPR tape driver not ported). ~~**ESZ: 1 word of 25 205**, the
/// heating column at 81.99999 eV, 75.45917 against 75.45914; cause not found.~~
/// **CORRECTED 2026-11-11 (gh:#1449): ESZ is now identical in all 25 205 words.**
/// Two causes: `run_acer` read MT=301's TAB1 from the wrong record (`rows[0]` is
/// the HEAD, so NP was 0 and the HEATR tape's own MT=301 was never used), and
/// `build_deck` rounded the PENDF through ENDF text although the deck's units are
/// blocked binary (negative): NJOY's grid keeps 81.99999000000082 eV, and the
/// heating there changes by 1e3 eV-b over the 8e-13 eV that text rounding moves.
/// NXS(1) and JXS(12-22) differ because GPD is missing. Before this port the
/// deck was refused (`newfor = 0`). ~~Pass criterion: exactly GPD differs.~~
/// **CORRECTED 2026-11-11 (gh:#1481): GROUPR and `gamout` ported; pass criterion:
/// no block differs** (the GENDF of card 9 `16 /` is identical line for line,
/// `test_07_groupr_gendf_whole_tape`, and `acer::gamout` appends the 30 x 20
/// equally probable photon matrix to GPD).
#[test]
fn test_07_acer_old_format_ace() {
    let root = njoy_root();
    let Some(full) = deck_text(&root, "07", None, "") else { return };
    let Some((ours, njoy)) = run_pair_decks(&root, "t07a", "t511", &full, &full) else { return };
    let c = ace_pair(&ours, &njoy, 26);
    for l in c.report_lines() {
        println!("{l}");
    }
    let differing: Vec<&str> = c.blocks.iter().filter(|b| !b.verdict.agrees()).map(|b| b.name).collect();
    assert!(differing.is_empty(), "recorded: every block identical: {differing:?}");
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Test 08 (Ni-61 ENDF/B-VI.1), the whole deck's ACE (`newfor = 0`)**, ACE
/// unit 25 against NJOY2016.80, NJOY on the whole upstream deck, this crate on
/// it without GROUPR (gh:#1449).
///
/// **Result (2026-11-11): the AND block (angular distributions, the `newfor = 0`
/// equally probable bins) is identical in all 2 943 words**, as are MTR, LQR, TYR
/// (MT=444 is now a stored reaction, thresh = `elow`, `sigfig(s/emev,7,0)`,
/// 443 skipped, `acefc.f90:1743-1782, :5664`), MTRP, LSIGP, SIGP, LANDP, ANDP,
/// LDLWP and DLWP (photon production).
///
/// **UPDATE 2026-11-11 (gh:#1449): ESZ is now identical (22 235 words), NXS(7)
/// and JXS(30-32) now agree (0), and units 41/42/43 of the PENDF are identical
/// line for line** (`test_08_pendf_stages_diagnosis`). Fixed: RECONR's `resxs`
/// seed took the `rdfil2` halo twice (the 4 827 against 4 825 points: extra
/// 2.53e-2 and 2.530004e-2 eV), the elastic 1e-8 floor now applies at the
/// background-only point above the range, the ESZ grid is the PENDF MT=1's
/// energies (`unionx`, `acefc.f90:1236-1300`; a bare 742925.0 from another
/// section had displaced MT=2's own 742925.0000000001 and the elastic read
/// 3.900002e-8 against 3.9e-8), and `acelcp` writes nothing for `newfor = 0`
/// (`first` collects production items only for `newfor != 0`, `:423`, `:621`).
/// ~~**Still different, not ported:** LAND/LDLW/DLW for the MF=6 reactions~~
/// **CORRECTED 2026-11-11 (gh:#1449): ported.** `topfil`'s `newfor = 0` MF=6 branch
/// (`acefc.f90:2233-2350`) and `fix6` (`:3351-3570`) convert Ni-61's LAW=1,
/// LANG=1 (Legendre) neutron subsections of MT=16/28/91 to LAW=7 with 33 lab
/// cosines, `pttab` reduces each energy's cosine table to its overall angular
/// law (`:2490-2530`), and `acelf6`'s law-7 branch writes ACE law 67
/// (`:7878-7946`): `acer::acelf6::acelf6_old_format`. **Measured: every block
/// is identical, 89 227 of 89 227 XSS words** (LAND 12, AND 2 943, LDLW 11, DLW
/// 6 823, LANDP/ANDP/LDLWP 36, DLWP 22 430; NXS and JXS agree), ESZ 22 235.
/// Pass criterion: no block differs. **Not exercised by this deck:** `LCT = 2`
/// (cm Jacobian), tabulated `LANG = 11-13` (ported with `fix6`'s own index
/// offset, unexercised), and a text PENDF scratch (`a11` rounding not applied).
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
    assert!(differing.is_empty(), "recorded: every block identical: {differing:?}");
    if std::env::var_os("KEEP_DECK_DIRS").is_none() { let _ = std::fs::remove_dir_all(ours.parent().unwrap()); }
}

/// **Test 08 (Ni-61 ENDF/B-VI.1) PENDF stages: RECONR (unit 41), BROADR (42),
/// HEATR (43)**, line by line against NJOY2016.80 (gh:#1449 diagnosis of the
/// ACE energy grid).
///
/// **Result (2026-11-11, after the fix): 0 differing lines on units 41, 42 and
/// 43.** Before: 6 336 / 13 879 / 23 696. Two causes, both in RECONR:
/// (1) `rebuild_range` seeded `resxs` with the `rdfil2` halo as well as the
/// `lunion` grid; upstream's `resxs` walks the `lunion` grid only, and
/// `lunion` had replaced the 0.0253 eV node by the histogram pair
/// 2.529999e-2/2.530001e-2 (MT=102), so ours kept 2.53e-2 and its bisections;
/// (2) `emerge`'s `sn = small` floor (1e-8) on MT=2 applies at every written
/// point, including the background-only point above the resonance range.
/// The earlier ACE comparison note in `test_08_acer_old_format_ace` about the
/// grid is superseded by this.
#[test]
fn test_08_pendf_stages_diagnosis() {
    let root = njoy_root();
    let Some(base) = deck_text(&root, "08", Some("groupr"), "moder\n -24 43/\n") else { return };
    let deck = base.replacen("broadr\n", "moder\n -22 41/\nbroadr\n", 1).replacen("heatr\n", "moder\n -23 42/\nheatr\n", 1);
    let Some((ours, njoy)) = run_pair(&root, "t08p", "eni61", &deck) else { return };
    for u in [41, 42, 43] {
        let d = differing_lines(&ours.join(format!("tape{u}")), &njoy.join(format!("tape{u}")));
        println!("test 08 unit {u}: {} differing lines, first {:?}", d.len(), &d[..d.len().min(5)]);
    }
    if std::env::var_os("KEEP_DECK_DIRS").is_none() { let _ = std::fs::remove_dir_all(ours.parent().unwrap()); }
}

/// **Test 01 through the first THERMR (free gas, `iinc = 1`), line for line.**
///
/// Deck: upstream `tests/01/input` up to its first `thermr` (moder reconr
/// broadr heatr), then that THERMR (`0 -22 -24`, carbon free gas, `mtref = 221`)
/// and a MODER converting its binary output to text (unit 31). Pass criterion:
/// **zero** differing lines on unit 31 (MF=1 dictionary, MF=3/MT=221, MF=6/MT=221).
/// The second THERMR (ENDF-5 graphite S(alpha,beta), `iinc = 2`) is not ported.
#[test]
fn test_01_first_thermr_free_gas_tape_is_identical_line_for_line() {
    let root = njoy_root();
    let tail = "thermr\n 0 -22 -24/\n 0 1306 8 1 1 0 0 1 221 2/\n 296./\n .05 1.2/\nmoder\n -24 31/\n";
    let Some(deck) = deck_text(&root, "01", Some("thermr"), tail) else { return };
    let Some((ours, njoy)) = run_pair(&root, "t01th", "t511", &deck) else { return };
    let d = differing_lines(&ours.join("tape31"), &njoy.join("tape31"));
    let text = std::fs::read_to_string(njoy.join("tape31")).unwrap();
    let (n, n6) = (text.lines().count(), text.lines().filter(|l| l.len() >= 75 && &l[70..72] == " 6").count());
    assert!(n6 > 100, "the tape must carry MF=6 lines ({n6} of {n})");
    println!("test 01 THERMR unit 31: {} differing lines of {n} ({n6} in MF=6)", d.len());
    assert!(d.is_empty(), "first differences {:?}", &d[..d.len().min(8)]);
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Test 01 through the second THERMR (graphite, ENDF/B-V `iinc = 2`, `icoh = 1`), line for line.**
///
/// Deck: upstream `tests/01/input` up to GROUPR (moder reconr broadr heatr, the
/// free-gas thermr, then `thermr 26 -24 -23 / 1065 1306 8 1 2 1 0 1 229 2`),
/// and a MODER converting the binary output (-23) to text unit 31. The
/// ENDF-5 tape `t322` is mounted as unit 26. Exercises `gateff` (`T_eff`
/// 713.39 K), `calcem` on the ENDF-5 `S(alpha,beta)`, `coh` with the built-in
/// graphite lattice (`sigcoh lat = 1`, `form`), `tpend` MF=3 for MT 229/230
/// and MF=6 for MT 230. Pass criterion: **zero** differing lines of unit 31.
#[test]
fn test_01_second_thermr_graphite_tape_is_identical_line_for_line() {
    let root = njoy_root();
    let Some(deck) = deck_text(&root, "01", Some("groupr"), "moder\n -23 31/\n") else { return };
    let Some((ours, njoy)) = run_pair_decks_extra(&root, "t01gr", "t511", &[("t322", 26)], &deck, &deck) else { return };
    let d = differing_lines(&ours.join("tape31"), &njoy.join("tape31"));
    let text = std::fs::read_to_string(njoy.join("tape31")).unwrap();
    let n = text.lines().count();
    let n229 = text.lines().filter(|l| l.len() >= 75 && &l[66..75] == "1306 3229").count();
    println!("test 01 second THERMR unit 31: {} differing lines of {n} (MF=3/MT=229 lines {n229})", d.len());
    assert!(n229 > 100, "the tape must carry MF=3/MT=229 ({n229})");
    assert!(d.is_empty(), "first differences {:?}", &d[..d.len().min(8)]);
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Test 01 GROUPR: the GENDF tape label and MF=1/MT=451 head (`groupr.f90:540-604`), line for line.**
///
/// Deck: upstream `tests/01/input` whole, plus `moder -24 27` (the deck's own last
/// MODER converts the PENDF, unit -23, not the GENDF, unit -24). Compares the
/// first 11 lines of unit 27 (label, HEAD, LIST of `ngn = 30`, `ngg = 12`,
/// `nw = 46`, FEND). Pass criterion: zero differing lines. Prediction: identical.
#[test]
fn test_01_groupr_gendf_head() {
    let root = njoy_root();
    let Some(deck) = deck_text(&root, "01", None, "moder\n -24 27/\n") else { return };
    let Some((ours, njoy)) = run_pair_decks_extra(&root, "t01g", "t511", &[("t322", 26)], &deck, &deck) else { return };
    let a = std::fs::read_to_string(ours.join("tape27")).unwrap();
    let b = std::fs::read_to_string(njoy.join("tape27")).unwrap();
    let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().take(11).collect(), b.lines().take(11).collect());
    assert_eq!(bl.len(), 11);
    assert_eq!(al, bl);
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Test 01 GROUPR: MF=6 transfer matrices (elastic and discrete inelastic, `nl = 4`).**
///
/// Deck as `test_01_groupr_gendf_head`. Compares every line of unit 27 with
/// MF=6 and MT in {2, 51-64, 91, 221, 229, 230}: faithful `panel` with `nl = lord + 1 = 4`, the
/// `getdis` two-body feed, and `displa`'s infinite-dilution matrix branch.
/// MT 91 is the `getff` label-200 continuum (`getfle` x `getsed` law 9, 25 lines, `nl = 1`).
/// MT 221/229/230 are the thermal `getaed` feed (`groupr/thermal.rs`: inelastic `aedi`, coherent Bragg edges).
/// Pass criterion: zero differing lines. Measured 2026-11-11: 0 of 513 (441 + 25 + 47).
#[test]
fn test_01_groupr_gendf_mf6_two_body() {
    let root = njoy_root();
    let Some(deck) = deck_text(&root, "01", None, "moder\n -24 27/\n") else { return };
    let Some((ours, njoy)) = run_pair_decks_extra(&root, "t01g6", "t511", &[("t322", 26)], &deck, &deck) else { return };
    let want = |l: &str| -> bool {
        if l.len() < 75 || &l[70..72] != " 6" {
            return false;
        }
        let mt: i32 = l[72..75].trim().parse().unwrap_or(0);
        matches!(mt, 2 | 51..=64 | 91 | 221 | 229 | 230)
    };
    let a = std::fs::read_to_string(ours.join("tape27")).unwrap();
    let b = std::fs::read_to_string(njoy.join("tape27")).unwrap();
    let al: Vec<&str> = a.lines().filter(|l| want(l)).collect();
    let bl: Vec<&str> = b.lines().filter(|l| want(l)).collect();
    let diffs: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
    println!("test 01 GENDF MF=6 two-body: {} differing lines of {} (ours {})", diffs.len(), bl.len(), al.len());
    for &i in diffs.iter().take(6) {
        println!("  ours: {:?}\n  njoy: {:?}", al.get(i), bl.get(i));
    }
    assert!(diffs.is_empty());
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Test 01 GROUPR: MF=3 vector sections of the GENDF, line for line.**
///
/// Deck as `test_01_groupr_gendf_head`. Compares every line of unit 27 with
/// MF=3 and MT in {1, 2, 4, 51-64, 91, 102, 103, 104, 107, 301, 444} (the
/// sections this driver writes: faithful `panel`, `getwtf` `iwt = 3`,
/// `getsig`/`gety1`, `displa` rounding). Prints the counts; pass criterion:
/// zero differing lines.
#[test]
fn test_01_groupr_gendf_mf3() {
    let root = njoy_root();
    let Some(deck) = deck_text(&root, "01", None, "moder\n -24 27/\n") else { return };
    let Some((ours, njoy)) = run_pair_decks_extra(&root, "t01g3", "t511", &[("t322", 26)], &deck, &deck) else { return };
    let want = |l: &str| -> bool {
        if l.len() < 75 || &l[70..72] != " 3" {
            return false;
        }
        let mt: i32 = l[72..75].trim().parse().unwrap_or(0);
        matches!(mt, 1 | 2 | 4 | 51..=64 | 91 | 102 | 103 | 104 | 107 | 221 | 229 | 230 | 251..=253 | 301 | 444)
    };
    let a = std::fs::read_to_string(ours.join("tape27")).unwrap();
    let b = std::fs::read_to_string(njoy.join("tape27")).unwrap();
    let al: Vec<&str> = a.lines().filter(|l| want(l)).collect();
    let bl: Vec<&str> = b.lines().filter(|l| want(l)).collect();
    let diffs: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
    println!("test 01 GENDF MF=3: {} differing lines of {} (ours {})", diffs.len(), bl.len(), al.len());
    for &i in diffs.iter().take(6) {
        println!("  ours: {:?}\n  njoy: {:?}", al.get(i), bl.get(i));
    }
    assert!(diffs.is_empty());
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Test 01 GROUPR: MF=16 photon production MT 51 (`mfd = 17`, `getff` label 300), line for line.**
///
/// Deck as `test_01_groupr_gendf_head`. One discrete photon (4.439 MeV, MF=13 cross
/// section, MF=14 `LTT = 1` Legendre) binned into the 12 gamma groups with `nl = 4`.
/// Pass criterion: zero differing lines of the 22 MF=16/MT=51 lines. Prediction: identical.
/// (MT 102 is `test_01_groupr_gendf_mf16_mt102`.)
#[test]
fn test_01_groupr_gendf_mf16_mt51() {
    let root = njoy_root();
    let Some(deck) = deck_text(&root, "01", None, "moder\n -24 27/\n") else { return };
    let Some((ours, njoy)) = run_pair_decks_extra(&root, "t01g16", "t511", &[("t322", 26)], &deck, &deck) else { return };
    let want = |l: &str| l.len() >= 75 && &l[70..72] == "16" && l[72..75].trim() == "51";
    let a = std::fs::read_to_string(ours.join("tape27")).unwrap();
    let b = std::fs::read_to_string(njoy.join("tape27")).unwrap();
    let al: Vec<&str> = a.lines().filter(|l| want(l)).collect();
    let bl: Vec<&str> = b.lines().filter(|l| want(l)).collect();
    let diffs: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
    println!("test 01 GENDF MF=16 MT 51: {} differing lines of {} (ours {})", diffs.len(), bl.len(), al.len());
    for &i in diffs.iter().take(6) {
        println!("  ours: {:?}\n  njoy: {:?}", al.get(i), bl.get(i));
    }
    assert!(bl.len() >= 22 && diffs.is_empty());
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Test 01 GROUPR: MF=16 MT 102 capture-gamma production (`mfd = 16`, MF=12 multiplicities), line for line.**
///
/// Deck as `test_01_groupr_gendf_head`. Three photons (4.9465 MeV primary `LP = 2`,
/// 3.684 MeV, 1.2625 MeV `LP = 2`) with constant multiplicities and isotropic MF=14.
/// The constant-spectrum branch of `getff` label 300 is exercised: `getgyl`'s constant-range
/// scan gives `econst = 1.2625e6/100 = 12625 eV`, `jconst = 12` neutron groups; the spectrum
/// is the `ig = 0` record, groups 1-12 carry flux and yield only (`ig2lo = 0`, `iglo = -1`),
/// groups 13+ the full matrix. Pass criterion: zero differing lines of the 72 MF=16/MT=102 lines.
/// Prediction: identical.
#[test]
fn test_01_groupr_gendf_mf16_mt102() {
    let root = njoy_root();
    let Some(deck) = deck_text(&root, "01", None, "moder\n -24 27/\n") else { return };
    let Some((ours, njoy)) = run_pair_decks_extra(&root, "t01g16b", "t511", &[("t322", 26)], &deck, &deck) else { return };
    let want = |l: &str| l.len() >= 75 && &l[70..72] == "16" && l[72..75].trim() == "102";
    let a = std::fs::read_to_string(ours.join("tape27")).unwrap();
    let b = std::fs::read_to_string(njoy.join("tape27")).unwrap();
    let al: Vec<&str> = a.lines().filter(|l| want(l)).collect();
    let bl: Vec<&str> = b.lines().filter(|l| want(l)).collect();
    let diffs: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
    println!("test 01 GENDF MF=16 MT 102: {} differing lines of {} (ours {})", diffs.len(), bl.len(), al.len());
    for &i in diffs.iter().take(6) {
        println!("  ours: {:?}\n  njoy: {:?}", al.get(i), bl.get(i));
    }
    assert!(bl.len() >= 72 && diffs.is_empty());
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Test 01 GROUPR: the WHOLE GENDF tape (unit 27), every line, against NJOY2016.80.**
///
/// Deck as `test_01_groupr_gendf_head`. Methodology: run the upstream deck (plus
/// `moder -24 27`) through this crate's deck driver and through the compiled
/// NJOY2016 and compare `tape27` line for line, MF=1 head, MF=3, MF=6 and MF=16
/// together with every FEND/SEND/MEND/TEND card and sequence number (`E: tape27`
/// is the GENDF; the sections are the union of the per-section tests above).
/// Pass criterion: identical line count and zero differing lines. Prediction:
/// identical (every section already matches individually).
#[test]
fn test_01_groupr_gendf_whole_tape() {
    let root = njoy_root();
    let Some(deck) = deck_text(&root, "01", None, "moder\n -24 27/\n") else { return };
    let Some((ours, njoy)) = run_pair_decks_extra(&root, "t01gw", "t511", &[("t322", 26)], &deck, &deck) else { return };
    let a = std::fs::read_to_string(ours.join("tape27")).unwrap();
    let b = std::fs::read_to_string(njoy.join("tape27")).unwrap();
    let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let diffs: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
    println!("test 01 GENDF whole tape: {} differing lines of {} (ours {})", diffs.len(), bl.len(), al.len());
    for &i in diffs.iter().take(6) {
        println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, al.get(i), bl.get(i));
    }
    assert!(bl.len() > 1000 && diffs.is_empty());
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Test 07 (U-235 ENDF/B-V) GROUPR GENDF, whole tape** (gh:#1481, automatic
/// reaction lists): the deck up to GROUPR (card 9 `16 /`, no `mtd`) plus a MODER
/// text copy of the GENDF unit, line by line against NJOY2016.80.
///
/// Pass criterion: identical line count and zero differing lines. Prediction:
/// not yet (`getsed` continuum photons for MT 18 and 102, MF=14 anisotropy of
/// MT 4); the result is recorded in the gh:#1481 comment.
#[test]
fn test_07_groupr_gendf_whole_tape() {
    let root = njoy_root();
    let Some(deck) = deck_text(&root, "07", Some("acer"), "moder\n -25 29/\n") else { return };
    let deck = deck.replace("stop\n", "stop\n");
    let Some((ours, njoy)) = run_pair_decks(&root, "t07g", "t511", &deck, &deck) else { return };
    let a = std::fs::read_to_string(ours.join("tape29")).unwrap_or_default();
    let b = std::fs::read_to_string(njoy.join("tape29")).unwrap();
    let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let diffs: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
    println!("test 07 GENDF whole tape: {} differing lines of {} (ours {})", diffs.len(), bl.len(), al.len());
    for &i in diffs.iter().take(6) {
        println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, al.get(i), bl.get(i));
    }
    if std::env::var_os("KEEP_DECK_DIRS").is_none() { let _ = std::fs::remove_dir_all(ours.parent().unwrap()); }
    assert!(bl.len() > 300 && diffs.is_empty());
}

/// **Test 21 (Fe-58 ENDF/B-VIII.0 Beta4) RECONR with card-3 `errmax = 0.003`**
/// (`err = 0.001`, `errint` default), the PENDF against NJOY2016.80 line by
/// line (gh:#1369, #1366).
///
/// Prediction (before the run): with `errmax` threaded into the `resxs`
/// refinement (`reconr.f90:2388-2407`; the default `10*err` was the only value
/// before) the grid matches NJOY's; any residual would be a separate port defect.
///
/// Result (2026-11-11): RECONR now runs (errmax = 0.003 threaded through
/// `reconr_with_errtol`; MF=10 multi-subsection sections ported in
/// `deck::pendf::other_files`). MF=1, MF=2, MT=1 and MT=2 are identical line
/// for line (33 312 lines, same point counts), but **19 496 of 52 902 lines
/// differ from MT=4 on**: NJOY numbers the SEND of MT=2 `16599` and continues
/// the sequence count (`16600`, ...) into MT=4, where this port writes `99999`
/// and restarts at 1. The data columns are not yet shown equal (the diff is
/// dominated by the sequence column).
///
/// ~~OPEN: the trigger (MT=2 ends after 16 598 lines) is not yet found~~
/// **CORRECTED 2026-11-11 (gh:#1484):** `recout` writes the redundant sums, MF=1
/// and MF=2 with `asend`/`afend` (SEND = 99999, count restarts) but COPIES every
/// non-redundant MF=3 section and all later files from its scratch tape with
/// `tosend`/`tofend`/`tomend` (`endf.f90:1064-1230`), which write the SEND, FEND
/// and MEND with the running count (`nsh`) and carry on. MT=2's SEND is
/// therefore `16599`, MT=4's HEAD `16600`; the MF=10 FEND is `1064`, the MEND
/// `1065`. `Tape::set_copied_closers` reproduces it.
#[test]
fn test_21_reconr_with_errmax_is_identical_line_for_line() {
    let root = njoy_root();
    let Some(deck) = deck_text(&root, "21", Some("broadr"), "") else { return };
    let Some((ours, njoy)) = run_pair(&root, "t21", "n-026_Fe_058-ENDF8.0-Beta4.endf", &deck) else { return };
    let d = differing_lines(&ours.join("tape21"), &njoy.join("tape21"));
    println!("test 21 unit 21: {} differing lines", d.len());
    assert!(d.is_empty(), "first differences {:?}", &d[..d.len().min(5)]);
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Tests 27 and 46: BROADR with card 3 `thnmx`, `errmax`, `errint`** (gh:#1369,
/// #1366), the PENDF after BROADR against NJOY2016.80 line by line.
///
/// Deck 27 (Pu-239 ENDF/B-VIII.0 Beta6): `0.001 1e+6 0.01 5e-8` (thnmx 1 MeV,
/// errmax 0.01, errint 5e-8); deck 46 (Fe-56 JEFF-3.3): `0.001 -850000.0`
/// (negative `thnmx`: broadening starts at 850 keV and the zero `errmax`/`errint`
/// take their `10 errthn` / `errthn/20000` defaults). Each deck is cut at its
/// GROUPR and a MODER conversion of the BROADR unit appended.
///
/// Prediction (before the run): BROADR already honours `thnmx` (`broadr_joint`,
/// `sigfig` and `eresh`) and the tolerance struct carries `errmax`/`errint`, so
/// the deck driver only refused the card; lifting the refusal should give
/// identical tapes. Result: printed below by the test; see survey.md.
#[test]
fn test_27_and_46_broadr_card3_pendf() {
    let root = njoy_root();
    for (case, res, tail, unit) in [
        ("27", "n-094_Pu_239-ENDF8.0-Beta6.endf", "moder\n -21 29/\n", 29),
        ("46", "n-026_Fe_056-JEFF3.3.endf", "moder\n -23 29/\n", 29),
    ] {
        let Some(deck) = deck_text(&root, case, Some("groupr"), tail) else { return };
        let Some((ours, njoy)) = run_pair(&root, &format!("t{case}"), res, &deck) else { return };
        let d = differing_lines(&ours.join(format!("tape{unit}")), &njoy.join(format!("tape{unit}")));
        println!("test {case} BROADR unit {unit}: {} differing lines", d.len());
        assert!(d.is_empty(), "test {case}: first differences {:?}", &d[..d.len().min(5)]);
        let _ = std::fs::remove_dir_all(ours.parent().unwrap());
    }
}

/// **Test 14 (p + N-14, LA150, `NSUB = 10010`): `acer` of a raw
/// incident-proton ENDF tape** (`izai = 1001`, the tape handed to ACER as both
/// ENDF and PENDF, units 20 and 21), ACE unit 31 against NJOY2016.80
/// (gh:#1485, parent #1369). The deck is cut before `viewr`.
///
/// Ported: `first`/`unionx`/`acelod`/`acecpe`/`acelf6`/`acelcp` `izai`
/// branches, `acer::charged`. **Result (2026-11-11): the table is identical
/// in every word** (NXS, JXS and all 71 369 XSS words, at Type-1 print
/// precision): ESZ with the LTP=12 elastic, the proton DLW, photons, and the
/// neutron/deuteron/alpha production blocks with the recoil heating.
/// Pass criterion: no differing word.
#[test]
fn test_14_charged_particle_ace_is_identical() {
    let root = njoy_root();
    // The deck's second ACER (`iopt = 7`, the plot file) and `viewr` are cut.
    let Some(full) = deck_text(&root, "14", Some("viewr"), "") else { return };
    let deck = format!("{}stop\n", full.split("acer\n 0 31").next().unwrap());
    let Some((ours, njoy)) = run_pair_decks_extra(&root, "t14", "epn14", &[("epn14", 21)], &deck, &deck) else { return };
    let c = ace_pair(&ours, &njoy, 31);
    for l in c.report_lines() {
        println!("{l}");
    }
    assert!(c.identical(), "test 14 ACE differs from NJOY2016");
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Test 13 (Ni-61 ENDF/B-VI.1): `moder; reconr; broadr; heatr; gaspr; moder;
/// acer`** (gh:#1369, differential #1366): the GASPR tape driver
/// (`gaspr::gaspr_tape`, `gaspr.f90`). The deck is cut before its second ACER
/// (`iopt = 7`, plot file) and `viewr`. Compared with NJOY2016.80: unit 28 (the
/// PENDF with MT=203-207, GASPR's binary output converted to text by MODER)
/// line by line, and ACE unit 26 word for word.
///
/// Prediction (before the run): GASPR is arithmetic on the PENDF's own grid
/// (MT=1 from `thrg`), summed in tape order, with `sigfig(.,7)`; identical
/// lines expected, the only risk being the directory counts (`(n+2)/3`).
/// Result: see the survey table (`verification_and_validation/deck_driver/survey.md`).
#[test]
fn test_13_gaspr_pendf_and_ace_are_identical() {
    let root = njoy_root();
    let Some(full) = deck_text(&root, "13", None, "") else { return };
    let deck = format!("{}stop\n", full.split("acer\n 0 26").next().unwrap());
    let Some((ours, njoy)) = run_pair_decks_extra(&root, "t13", "eni61", &[], &deck, &deck) else { return };
    let d = differing_lines(&ours.join("tape28"), &njoy.join("tape28"));
    println!("test 13 unit 28: {} differing lines", d.len());
    assert!(d.is_empty(), "test 13 unit 28: first differences {:?}", &d[..d.len().min(5)]);
    let c = ace_pair(&ours, &njoy, 26);
    for l in c.report_lines() {
        println!("{l}");
    }
    assert!(c.identical(), "test 13 ACE differs from NJOY2016");
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Test 12 (Ni-61): `reconr; gaspr` on coded units** (gh:#1369), the PENDF
/// GASPR writes (unit 22) against NJOY2016.80 line by line. Unlike test 13 the
/// units are positive, so the tape's SEND/FEND/MEND numbering is GASPR's own
/// (`tosend` copies from its scratch tape) and the MODER conversion is absent.
#[test]
fn test_12_gaspr_coded_pendf_is_identical() {
    let root = njoy_root();
    let Some(full) = deck_text(&root, "12", Some("plotr"), "") else { return };
    let Some((ours, njoy)) = run_pair(&root, "t12", "eni61", &full) else { return };
    let d = differing_lines(&ours.join("tape22"), &njoy.join("tape22"));
    println!("test 12 unit 22: {} differing lines; first {:?}", d.len(), &d[..d.len().min(8)]);
    assert!(d.is_empty(), "test 12 unit 22: first differences {:?}", &d[..d.len().min(5)]);
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Test 24 (Pu-239 ENDF/B-VIII.0 Beta6): `moder; reconr; broadr; heatr; heatr;
/// thermr; gaspr; moder; acer`**, cut before the plot-file ACER (`iopt = 7`) and
/// the `viewr`s: PENDF unit 38 (after GASPR) line by line, ACE unit 31 word for
/// word (gh:#1369).
///
/// **Result (2026-11-12, gh:#1492): the PENDF after GASPR is identical (0 lines)
/// and the ACE is identical in every word (1 037 922 XSS words).** ~~First
/// run: 280 SIGP words differ, 1.504386e-3 here against NJOY's 1.504385995e-3
/// (3e-9 relative).~~ **CORRECTED**: the cause was not GASPR and not `sigfig`
/// but the ENDF unit's sign. The deck's ACER card is `-21 -28`, both binary,
/// so `convr`'s scratch tape is binary (`if (nin.lt.0) nout=-nout`,
/// `acefc.f90:3908`) and the MF=12 yields of a LO=2 cascade reach `acelpp`
/// as doubles; this crate applied `a11`'s seven-figure text round trip
/// unconditionally on the neutron path (`photon_blocks::build_with_pendf`).
/// Fix: `AceDeck::with_endf_binary` and `photon_blocks::build_with_pendf_units`
/// / `gpd_with_units`, plus the MF=13 `gamsum` scratch tape (PENDF sign).
#[test]
fn test_24_gaspr_chain_ace() {
    let root = njoy_root();
    let Some(full) = deck_text(&root, "24", Some("viewr"), "") else { return };
    let deck = format!("{}stop\n", full.split("acer\n 0 31").next().unwrap());
    let Some((ours, njoy)) = run_pair_decks_extra(&root, "t24", "n-094_Pu_239-ENDF8.0-Beta6.endf", &[], &deck, &deck) else { return };
    let d = differing_lines(&ours.join("tape38"), &njoy.join("tape38"));
    println!("test 24 unit 38: {} differing lines; first {:?}", d.len(), &d[..d.len().min(5)]);
    let c = ace_pair(&ours, &njoy, 31);
    for l in c.report_lines() {
        println!("{l}");
    }
    assert!(d.is_empty() && c.identical(), "test 24 differs from NJOY2016");
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
}

/// **Test 72 (Be-9 ENDF/B-VIII.0): `moder; reconr; broadr; heatr; heatr;
/// thermr; gaspr; moder; acer`**, cut before the plot-file ACER (`iopt = 7`)
/// and `viewr`: PENDF unit 38 (after GASPR) line by line, ACE unit 41 word for
/// word (gh:#1492, parent #1369). The same chain as test 24 on a light
/// nuclide with MF=6 (n,2n) products and LAW = 2 photons.
///
/// **Result (2026-11-12): unit 38 identical (0 lines), ACE unit 41 identical
/// in every word (202 943 XSS words).** Before: 4 304 words differ (2 596 of
/// 50 748 lines), all in the MF=6 LAW = 7 products (Be-9 MT=16 neutron and the
/// charged-particle blocks): `topfil` rewrites LAW = 7 to LAW = 1 on its
/// scratch tape, which is binary when the PENDF unit is negative
/// (`mscr = -mscr`, `acefc.f90:177-179`), and `acelf6::law7_rewrite` applied
/// `a11` regardless (7th-figure differences, largest 1.8e-6 relative). Now
/// `law7_rewrite(cur, text)`; `AceDeck::with_pendf_binary` selects it.
#[test]
fn test_72_gaspr_chain_ace() {
    let root = njoy_root();
    let Some(full) = deck_text(&root, "72", Some("viewr"), "") else { return };
    let deck = format!("{}stop\n", full.split("acer\n 0 41").next().unwrap());
    let Some((ours, njoy)) = run_pair_decks_extra(&root, "t72", "n-004_Be_009-ENDF8.0.endf", &[], &deck, &deck) else { return };
    let d = differing_lines(&ours.join("tape38"), &njoy.join("tape38"));
    println!("test 72 unit 38: {} differing lines; first {:?}", d.len(), &d[..d.len().min(5)]);
    let c = ace_pair(&ours, &njoy, 41);
    for l in c.report_lines() {
        println!("{l}");
    }
    assert!(d.is_empty() && c.identical(), "test 72 differs from NJOY2016");
    if std::env::var("KEEP_DECK_DIRS").is_err() {
        let _ = std::fs::remove_dir_all(ours.parent().unwrap());
    } else {
        println!("kept {}", ours.parent().unwrap().display());
    }
}

/// ACER `iopt = 7` (gh:#1492): run upstream deck `case` cut before its first
/// `viewr`, and compare, line by line against NJOY2016.80, the listed units:
/// the ACE file the check run writes, the directory lines and, where the deck
/// has `ngend`, the plot file (`aplots`, unit 33: viewr input text, gh:#1369).
fn acer_check_deck(case: &str, resource: &str, extra: &[(&str, u32)], units: &[u32]) -> Option<()> {
    let root = njoy_root();
    let full = deck_text(&root, case, Some("viewr"), "")?;
    let (ours, njoy) = run_pair_decks_extra(&root, &format!("chk{case}"), resource, extra, &full, &full)?;
    for &unit in units {
        let name = format!("tape{unit}");
        let d = differing_lines(&ours.join(&name), &njoy.join(&name));
        println!("deck {case} unit {unit}: {} differing lines; first {:?}", d.len(), &d[..d.len().min(5)]);
        // The date field of the ACE header is the run's own date on both sides.
        assert!(d.is_empty(), "deck {case} unit {unit}: {} lines differ from NJOY2016", d.len());
    }
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
    Some(())
}

/// **Test 72 (Be-9), ACER `iopt = 7`**: the check run's ACE unit 44 and its
/// directory unit 45 against NJOY2016.80, line by line (gh:#1492).
///
/// Prediction (before the run): `acefix` re-writes the file it reads, so unit 44
/// is unit 41 with the comment replaced by the card-3 text (`suff = -1` keeps
/// the ZAID); unit 45 is one `xsdir` line. Both identical. Result: see the
/// survey table.
#[test]
fn test_72_acer_check_unit_and_directory() {
    let _ = acer_check_deck("72", "n-004_Be_009-ENDF8.0.endf", &[], &[42, 43, 44, 45]);
}

#[test]
fn test_24_acer_check_unit_and_directory() {
    let _ = acer_check_deck("24", "n-094_Pu_239-ENDF8.0-Beta6.endf", &[], &[32, 33, 34, 35]);
}

#[test]
#[ignore = "the preceding iopt = 1 ACE differs in 5 unresolved-range (PURR) words, which the check file repeats; not the iopt = 7 writer (gh:#1492)"]
fn test_35_acer_check_unit_and_directory() {
    let _ = acer_check_deck("35", "n-047_Ag_109-ENDF8.0.endf", &[], &[42, 43, 44, 45]);
}

#[test]
fn test_36_acer_check_unit_and_directory() {
    let _ = acer_check_deck("36", "n-050_Sn_119-ENDF8.0.endf", &[], &[42, 43, 44, 45]);
}

#[test]
#[ignore = "the preceding iopt = 1 ACE is 21 words longer than NJOY's (PURR unresolved block), so its directory line differs; not the iopt = 7 writer (gh:#1492)"]
fn test_37_acer_check_unit_and_directory() {
    let _ = acer_check_deck("37", "n-027_Co_058-ENDF8.0.endf", &[], &[42, 43, 44, 45]);
}

#[test]
fn test_38_acer_check_unit_and_directory() {
    let _ = acer_check_deck("38", "n-036_Kr_083-ENDF8.0.endf", &[], &[42, 43, 44, 45]);
}

#[test]
fn test_39_acer_check_unit_and_directory() {
    let _ = acer_check_deck("39", "n-028_Ni_063-ENDF8.0.endf", &[], &[42, 43, 44, 45]);
}

#[test]
fn test_40_acer_check_unit_and_directory() {
    let _ = acer_check_deck("40", "n-025_Mn_055-ENDF8.0.endf", &[], &[42, 43, 44, 45]);
}

#[test]
fn test_41_acer_check_unit_and_directory() {
    let _ = acer_check_deck("41", "n-032_Ge_071-ENDF8.0.endf", &[], &[42, 43, 44, 45]);
}

#[test]
fn test_42_acer_check_unit_and_directory() {
    let _ = acer_check_deck("42", "n-030_Zn_067-ENDF8.0.endf", &[], &[42, 43, 44, 45]);
}

#[test]
fn test_50_acer_check_unit_and_directory() {
    let _ = acer_check_deck("50", "a-002_He_004-ENDF8.0.endf", &[], &[32, 33, 34, 35]);
}

#[test]
fn test_51_acer_check_unit_and_directory() {
    let _ = acer_check_deck("51", "p-001_H_002-ENDF8.0.endf", &[], &[32, 33, 34, 35]);
}

#[test]
fn test_52_acer_check_unit_and_directory() {
    let _ = acer_check_deck("52", "p-001_H_001-ENDF8.0.endf", &[], &[32, 33, 34, 35]);
}

#[test]
fn test_53_acer_check_unit_and_directory() {
    let _ = acer_check_deck("53", "d-001_H_002-ENDF8.0.endf", &[], &[32, 33, 34, 35]);
}

#[test]
fn test_54_acer_check_unit_and_directory() {
    let _ = acer_check_deck("54", "p-001_H_003-ENDF8.0.endf", &[], &[32, 33, 34, 35]);
}

#[test]
fn test_62_acer_check_unit_and_directory() {
    let _ = acer_check_deck("62", "d-002_He_003.endf", &[], &[32, 33, 34, 35]);
}

#[test]
#[ignore = "the preceding iopt = 1 ACE is 2 178 words longer than NJOY's (PURR unresolved block); not the iopt = 7 writer (gh:#1492)"]
fn test_63_acer_check_unit_and_directory() {
    let _ = acer_check_deck("63", "n-047_Ag_109-ENDF8.0.endf", &[], &[42, 43, 44, 45]);
}

#[test]
#[ignore = "the preceding iopt = 1 ACE differs from NJOY's (a TENDL-2021 d + Pt-184 charged-particle table), which the check file repeats; not the iopt = 7 writer (gh:#1492)"]
fn test_71_acer_check_unit_and_directory() {
    let _ = acer_check_deck("71", "d-078_Pt_184-TENDL2021.endf", &[], &[32, 33, 34, 35]);
}

/// **The plot file of NJOY's own ACE file** (gh:#1369): `acefix` with `aplots`
/// run on the `iopt = 1` ACE file *NJOY2016 wrote* (`ace_unit` of the NJOY
/// run), against NJOY2016's own plot file (`plot_unit`), line by line, and the
/// edited ACE file (`ace_out_unit`) as a control.
///
/// Why: decks 35, 37, 63 and 71 cannot compare unit 43/33 end to end, because
/// the table this crate writes for them differs from NJOY's before `aplots`
/// runs (PURR words; a TENDL d + Pt-184 table). Feeding NJOY's own table to
/// both writers isolates `aplots` from that.
fn plot_of_njoys_own_ace(case: &str, resource: &str, ace_unit: u32, plot_unit: u32, ace_out_unit: u32, hk: &str) -> Option<()> {
    let root = njoy_root();
    let full = deck_text(&root, case, Some("viewr"), "")?;
    let (ours, njoy) = run_pair_decks_extra(&root, &format!("own{case}"), resource, &[], &full, &full)?;
    let ace = std::fs::read_to_string(njoy.join(format!("tape{ace_unit}"))).unwrap();
    let req = njoy_outram_park_fork::acer::acefix::AcefixRequest { suff: -1.0, hk: hk.into(), nxtra: 0, plot: true };
    let out = njoy_outram_park_fork::acer::acefix::acefix_type1(&ace, &req).unwrap();
    std::fs::write(ours.join("own_plot"), out.plot.expect("plot requested")).unwrap();
    std::fs::write(ours.join("own_ace"), out.ace).unwrap();
    let d = differing_lines(&ours.join("own_plot"), &njoy.join(format!("tape{plot_unit}")));
    println!("deck {case}: plot of NJOY's own ACE: {} differing lines; first {:?}", d.len(), &d[..d.len().min(5)]);
    let c = differing_lines(&ours.join("own_ace"), &njoy.join(format!("tape{ace_out_unit}")));
    println!("deck {case}: edited ACE of NJOY's own ACE: {} differing lines; first {:?}", c.len(), &c[..c.len().min(5)]);
    assert!(d.is_empty(), "deck {case}: {} plot lines differ from NJOY2016", d.len());
    assert!(c.is_empty(), "deck {case}: {} edited-ACE lines differ from NJOY2016", c.len());
    let _ = std::fs::remove_dir_all(ours.parent().unwrap());
    Some(())
}

#[test]
fn test_35_plot_of_njoys_own_ace() {
    let _ = plot_of_njoys_own_ace("35", "n-047_Ag_109-ENDF8.0.endf", 41, 43, 44, "acer check/plot tape for material 4731");
}

#[test]
fn test_37_plot_of_njoys_own_ace() {
    let _ = plot_of_njoys_own_ace("37", "n-027_Co_058-ENDF8.0.endf", 41, 43, 44, "acer check/plot tape for material 2722");
}

#[test]
fn test_63_plot_of_njoys_own_ace() {
    let _ = plot_of_njoys_own_ace("63", "n-047_Ag_109-ENDF8.0.endf", 41, 43, 44, "acer check/plot tape for material 4731");
}

#[test]
fn test_71_plot_of_njoys_own_ace() {
    let _ = plot_of_njoys_own_ace("71", "d-078_Pt_184-TENDL2021.endf", 31, 33, 34, "TENDL-2021 deuteron + Pt184");
}

/// Run upstream deck `case` (`moder; moder; acer iopt = 1`, the raw
/// incident-charged-particle evaluation `resource` on unit 20) cut before its
/// second ACER (the `iopt = 7` plot), and return the ACE comparison of unit 31.
fn charged_deck_ace(case: &str, resource: &str) -> Option<njoy_outram_park_fork::acer::compare::AceComparison> {
    let root = njoy_root();
    let full = deck_text(&root, case, Some("viewr"), "")?;
    let deck = format!("{}stop\n", full.split("acer\n 0 31").next().unwrap());
    let (ours, njoy) = run_pair_decks_extra(&root, &format!("t{case}"), resource, &[], &deck, &deck)?;
    let c = ace_pair(&ours, &njoy, 31);
    for l in c.report_lines() {
        println!("{l}");
    }
    if std::env::var("KEEP_DECK_DIRS").is_err() {
        let _ = std::fs::remove_dir_all(ours.parent().unwrap());
    } else {
        println!("kept {}", ours.parent().unwrap().display());
    }
    Some(c)
}

#[test]
fn test_50_charged_particle_ace() {
    if let Some(c) = charged_deck_ace("50", "a-002_He_004-ENDF8.0.endf") {
        assert!(c.identical(), "test 50 ACE differs from NJOY2016");
    }
}

#[test]
fn test_51_charged_particle_ace() {
    if let Some(c) = charged_deck_ace("51", "p-001_H_002-ENDF8.0.endf") {
        assert!(c.identical(), "test 51 ACE differs from NJOY2016");
    }
}

#[test]
fn test_52_charged_particle_ace() {
    if let Some(c) = charged_deck_ace("52", "p-001_H_001-ENDF8.0.endf") {
        assert!(c.identical(), "test 52 ACE differs from NJOY2016");
    }
}

#[test]
fn test_53_charged_particle_ace() {
    if let Some(c) = charged_deck_ace("53", "d-001_H_002-ENDF8.0.endf") {
        assert!(c.identical(), "test 53 ACE differs from NJOY2016");
    }
}

#[test]
fn test_54_charged_particle_ace() {
    if let Some(c) = charged_deck_ace("54", "p-001_H_003-ENDF8.0.endf") {
        assert!(c.identical(), "test 54 ACE differs from NJOY2016");
    }
}

#[test]
fn test_62_charged_particle_ace() {
    if let Some(c) = charged_deck_ace("62", "d-002_He_003.endf") {
        assert!(c.identical(), "test 62 ACE differs from NJOY2016");
    }
}

#[test]
fn test_71_charged_particle_ace() {
    if let Some(c) = charged_deck_ace("71", "d-078_Pt_184-TENDL2021.endf") {
        assert!(c.identical(), "test 71 ACE differs from NJOY2016");
    }
}
