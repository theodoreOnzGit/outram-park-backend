// SPDX-License-Identifier: AGPL-3.0-only
//! **COVR's plot option from a deck, whole decks against NJOY2016.80** (gh:#1369).
//!
//! # Methodology
//!
//! Upstream decks 18 (Cf-252 decay data, `mfcov = 35`, text ERRORR unit 28,
//! `epmin = 1e3`), 27 (Pu-239 ENDF/B-VIII.0 beta 6, two ERRORR/COVR/VIEWR runs),
//! 34 (Pu-240 ENDF/B-VIII.0) and 47 (Pu-239, GROUPR with MF=6 chi, two runs) are
//! run **whole** by the NJOY2016.80 binary (`NJOY2016_ROOT/build/njoy`) and by
//! [`run_deck`], each on the same `tests/resources` tape mounted as unit 20.
//! Compared line by line: every COVR plot-command unit (VIEWR's input), every
//! VIEWR PostScript unit, and the decks' own MODER copies of the ERRORR tapes.
//! Pass criterion: zero differing lines on every unit, and the deck runs to its
//! `stop` (`DeckRun::blocked == None`). A case skips with a message when the
//! NJOY clone is absent.
//!
//! # Prediction (posted on gh:#1369 before the code)
//!
//! Every COVR plot file identical on its first run; if not, the last figure of a
//! `1pe13.4` word. VIEWR then identical, so all four decks close whole.
//!
//! # Results (2026-11-13, NJOY2016.80 `ac5adf5`, gfortran 13.3, release)
//!
//! See each test. All four decks: every compared unit identical, first run.
//!
//! # ERRORR `mfcov = 34`, `31`, `33` from a GENDF, and `covadd` (2026-11-13)
//!
//! Decks 46, 65, 16, 04, 15 and 20 are run whole the same way. Predictions were
//! posted on gh:#1369 before each change, except deck 04's (MF=31), which was run
//! before its prediction was written; that is recorded there.
//! - 46 (MF=34 from a binary GENDF), 65 (MF=34 with six sub-subsections, COVR,
//!   VIEWR), 16 (MF=34 from a PENDF through `grpav4`'s ENDF branch) and 15 (MF=31,
//!   33 and 34 from a text GENDF): every unit identical on the first run.
//! - 04 (MF=31 with `nin` copied ahead): identical on the first run.
//! - 20 (`covadd`, Cl-35 R-matrix limited): the first run differed in two places,
//!   both fixed from upstream. RECONR's MF=2/MT=151 stub took `spin` 0 where
//!   `rdsammy` sets the elastic pair's `IB` (1.5, `samm.f90:1084`). MF=33's first
//!   section's sequence numbers start at 324 because `rdumrd2` copies MF=2/MT=151's
//!   323 lines to a coded scratch unit and NJOY's `nsh` counter is shared by all
//!   units. After both, every unit is identical.

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

mod common;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// Run upstream deck `case` whole in NJOY2016.80 and in this crate, on `resource`
/// as unit 20; compare `units` line by line. `None`: skipped.
fn whole_deck_on(case: &str, resource: &str, unit_in: u32, units: &[u32]) -> Option<Vec<(u32, usize, usize)>> {
    edited_deck_on(case, resource, unit_in, units, |d| d.to_string())
}

/// [`whole_deck_on`] on upstream deck `case` as rewritten by `edit` (a deck of our
/// own built from it; gh:#1371).
fn edited_deck_on(case: &str, resource: &str, unit_in: u32, units: &[u32], edit: impl Fn(&str) -> String) -> Option<Vec<(u32, usize, usize)>> {
    let root = njoy_root();
    let bin = root.join("build/njoy");
    let res = root.join("tests/resources").join(resource);
    let input = root.join(format!("tests/{case}/input"));
    for p in [&bin, &res, &input] {
        if !p.exists() {
            eprintln!("skip: {} not present (set NJOY2016_ROOT)", p.display());
            return None;
        }
    }
    let deck = edit(&std::fs::read_to_string(&input).unwrap());
    // One directory per call: edited decks of the same upstream `case` (I1, I2
    // both edit deck 05) run in parallel threads of one process, so `case` and
    // the pid alone collide.
    static RUN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run_id = RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("njoy_deck_covr{case}_{}_{run_id}", std::process::id()));
    let (ours, theirs) = (tmp.join("ours"), tmp.join("njoy"));
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::copy(&res, d.join(format!("tape{unit_in}"))).unwrap();
    }
    std::fs::write(theirs.join("deck"), &deck).unwrap();
    let (ok, date) = common::run_njoy_dated(&bin, &theirs); // gh:#1549
    assert!(ok, "NJOY2016 binary failed on deck {case}");
    let run = run_deck(&deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date.clone()) }).unwrap();
    assert_eq!(run.blocked, None, "deck {case} must run to its end");
    let mut out = Vec::new();
    for &u in units {
        let read = |d: &Path| std::fs::read_to_string(d.join(format!("tape{u}"))).unwrap_or_default();
        let (a, b) = (read(&ours), read(&theirs));
        let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
        let diff: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
        println!("deck {case} unit {u}: {} differing lines of {}", diff.len(), bl.len());
        for &i in diff.iter().take(4) {
            println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, al.get(i), bl.get(i));
        }
        out.push((u, diff.len(), bl.len()));
    }
    if std::env::var_os("DECK_KEEP").is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    Some(out)
}

/// [`whole_deck_on`] with the resource on unit 20.
fn whole_deck(case: &str, resource: &str, units: &[u32]) -> Option<Vec<(u32, usize, usize)>> {
    whole_deck_on(case, resource, 20, units)
}

fn assert_identical(case: &str, r: Option<Vec<(u32, usize, usize)>>) {
    let Some(r) = r else { return };
    for (u, d, n) in r {
        assert!(n > 20 && d == 0, "deck {case} unit {u}: {d} of {n} lines differ");
    }
}

/// **Deck 34 (Pu-240): COVR plot unit 33 and VIEWR unit 43, whole deck.**
#[test]
fn test_34_covr_plot_and_viewr_whole_deck() {
    assert_identical("34", whole_deck("34", "n-094_Pu_240-ENDF8.0.endf", &[33, 43]));
}

/// **Deck 18 (Cf-252, text ERRORR unit 28, `epmin = 1e3`): units 28, 38, 39, whole deck.**
#[test]
fn test_18_covr_plot_and_viewr_whole_deck() {
    assert_identical("18", whole_deck("18", "DCf252", &[28, 38, 39]));
}

/// **Deck 27 (Pu-239, two ERRORR/COVR/VIEWR runs, the second VIEWR inheriting the
/// first's state): units 36, 37, 46, 47, 56, 57, whole deck.**
#[test]
fn test_27_covr_plot_and_viewr_whole_deck() {
    assert_identical("27", whole_deck("27", "n-094_Pu_239-ENDF8.0-Beta6.endf", &[36, 37, 46, 47, 56, 57]));
}

/// **Deck 47 (Pu-239, GROUPR MF=6 chi, two runs): units 45-48 and 55-58, whole deck.**
#[test]
fn test_47_covr_plot_and_viewr_whole_deck() {
    assert_identical("47", whole_deck("47", "n-094_Pu_239-ENDF8.0-Beta6.endf", &[45, 46, 47, 48, 55, 56, 57, 58]));
}

/// **Deck 05 (`t511`, MAT 1306): ERRORR `mfcov = 33` from a PENDF (unit 33), COVR's
/// plot of every MT pair (unit 34) and VIEWR (unit 35), whole deck.**
#[test]
fn test_05_errorr_mf33_covr_viewr_whole_deck() {
    assert_identical("05", whole_deck_on("05", "t511", 30, &[33, 34, 35]));
}

/// A TAB1 weight for ERRORR's card 13: two regions (INT 2 to point 2, then INT 5),
/// three points, no trailing zero.
const CARD13: &str = " 0 0 0 0 2 3 2 2 3 5 1e-5 1. 1e5 2. 2e7 .5/\n";

/// **`E1`: deck 05 with `iwt = 1`** (gh:#1371): ERRORR `mfcov = 33` from a PENDF,
/// the weight read from card 13 by `egnwtf` (`errorr.f90:9916-9932`) and used by
/// `grpav`. The ERRORR (33), COVR (34) and VIEWR (35) units are compared with
/// NJOY2016.80 on the same deck. Prediction (gh:#1371, before the run): identical.
/// **Result (2026-11-13): 33 (21 471 lines), 34 (78 796) and 35 (199 652)
/// identical.** Control: NJOY's own unit 33 differs from deck 05's (`iwt = 2`) in
/// 210 162 bytes, so the weight reaches the output.
#[test]
fn e1_deck_05_with_errorr_iwt1_card13() {
    let r = edited_deck_on("05", "t511", 30, &[33, 34, 35], |d| {
        let e = d.replacen(" 1306 -1 2 1/", " 1306 -1 1 1/", 1).replacen(" 1e-5 2e7/\n", &format!(" 1e-5 2e7/\n{CARD13}"), 1);
        assert!(e.contains(" 1306 -1 1 1/") && e.contains(CARD13), "deck 05 was not edited");
        e
    });
    assert_identical("e1", r);
}

/// **`E2`: deck 46 with `iwt = 1`** (gh:#1371): ERRORR `mfcov = 34` from a binary
/// GENDF calls `egnwtf` twice, in `colaps` (`:9122`, which reads card 13 and does
/// not use it) and in `grpav4` (`:5467`), so card 13 is given twice. Unit 34 is
/// compared with NJOY2016.80 on the same deck. Prediction (gh:#1371): identical.
/// **Result (2026-11-13): unit 34 identical, 8 002 lines.** What this checks is the
/// card count only: NJOY's unit 34 here is byte-identical to deck 46's (`iwt = 2`),
/// so the weight does not reach a GENDF-route MF=34 tape, but a single read would
/// leave the second card for the deck reader and stop the run.
#[test]
fn e2_deck_46_with_errorr_iwt1_card13_twice() {
    let r = edited_deck_on("46", "n-026_Fe_056-JEFF3.3.endf", 20, &[34], |d| {
        let e = d.replacen("2631 1 2 0 1 /", "2631 1 1 0 1 /", 1).replacen("2.000E+07/\nstop", &format!("2.000E+07/\n{CARD13}{CARD13}stop"), 1);
        assert!(e.contains("2631 1 1 0 1 /") && e.contains(&format!("{CARD13}{CARD13}stop")), "deck 46 was not edited");
        e
    });
    assert_identical("e2", r);
}

/// Deck 05 with card 7 `iread = 1` and `cards` (8, 8a, 8b, 9) after it.
fn deck_05_iread1(d: &str, cards: &str) -> String {
    let e = d.replacen("\n 0 33/\n", &format!("\n 1 33/\n{cards}"), 1);
    assert!(e.contains(&format!("\n 1 33/\n{cards}")) && e.contains(" -31 -32 0 33/"), "deck 05 was not edited");
    e
}

/// **`I1`: deck 05 with `iread = 1`, card 8a `2 102`, `nek = 0`** (gh:#1371).
/// `gridd` and `covcal` skip every MF=33 section not on card 8a
/// (`errorr.f90:1154-1157`, `:1913-1918`); a subsection whose `mt1` is not on it
/// has `iok = 0` (`:1220-1225`); `akxy` is the identity (`:883-891`). Units 33
/// (ERRORR), 34 (COVR) and 35 (VIEWR) against NJOY2016.80 on the same deck.
/// Prediction (gh:#1371, before the run): identical. Results: see the
/// deck-driver README §37.
#[test]
fn i1_deck_05_with_errorr_iread1() {
    let r = edited_deck_on("05", "t511", 30, &[33, 34, 35], |d| deck_05_iread1(d, " 2 0/\n 2 102/\n"));
    assert_identical("i1", r);
}

/// **`I2`: deck 05 with `iread = 1`, card 8a `1 2 102`, `nek = 1`**, card 9
/// making MT 1 the sum of MT 2 and MT 102 over `1e-5..2e7` eV (gh:#1371): cards
/// 8b and 9 (`errorr.f90:892-901`) replace the file's `LTY = 0` formulas, which
/// `gridd` reads past (`:1299`). Prediction (gh:#1371): identical. Results: see
/// the deck-driver README §37.
#[test]
fn i2_deck_05_with_errorr_iread1_card9() {
    let r = edited_deck_on("05", "t511", 30, &[33, 34, 35], |d| deck_05_iread1(d, " 3 1/\n 1 2 102/\n 1e-5 2e7/\n 0 1 1/\n 0 1 0/\n 0 0 1/\n"));
    assert_identical("i2", r);
}

/// **Deck 46 (Fe-56 JEFF-3.3, 238 groups): ERRORR `mfcov = 34` from a binary GENDF
/// (unit 34: MF=1, MF=3/MT=251 mubar, MF=34/MT=251), whole deck.**
#[test]
fn test_46_errorr_mf34_whole_deck() {
    assert_identical("46", whole_deck("46", "n-026_Fe_056-JEFF3.3.endf", &[34]));
}

/// **Deck 65 (U-235 ENDF/B-VIII.0, MF=34/MT=2 with six sub-subsections): ERRORR
/// `mfcov = 34` from a text GENDF (unit 41), COVR (51) and VIEWR (61), whole deck.**
#[test]
fn test_65_errorr_mf34_covr_viewr_whole_deck() {
    assert_identical("65", whole_deck("65", "n-092_U_235-ENDF8.0.endf", &[41, 51, 61]));
}

/// **Deck 16 (U-238 JENDL-3.3, from a PENDF): ERRORR `mfcov = 33` (26) and `34`
/// (27, `grpav4` from MF=4 LTT=1), their COVR plots (36, 37) and VIEWR (46, 47),
/// whole deck.**
#[test]
fn test_16_errorr_mf33_mf34_covr_viewr_whole_deck() {
    assert_identical("16", whole_deck("16", "J33U238", &[26, 27, 36, 37, 46, 47]));
}

/// **Deck 04 (`t511` U-235): ERRORR `mfcov = 33` from a PENDF (23), GROUPR nubar
/// (24), then ERRORR `mfcov = 31` from that GENDF with `nin = 23` copied ahead
/// (25), whole deck.**
#[test]
fn test_04_errorr_mf31_whole_deck() {
    assert_identical("04", whole_deck("04", "t511", &[23, 24, 25]));
}

/// **Deck 15 (U-238 JENDL-3.3, text GENDF unit 91): ERRORR `mfcov = 31`, `33` and
/// `34` from the GENDF (25-27), three COVR plots (35-37) and VIEWR (45-47), whole deck.**
#[test]
fn test_15_errorr_mf31_mf33_mf34_from_gendf_whole_deck() {
    assert_identical("15", whole_deck("15", "J33U238", &[25, 26, 27, 35, 36, 37, 45, 46, 47, 91]));
}

/// **Deck 20 (Cl-35 R-matrix limited): ERRORR `nendf = 999` (`covadd`, unit 21), then
/// RECONR, BROADR, GROUPR, ERRORR `mfcov = 33` from the GENDF with MF=32, COVR and
/// VIEWR, whole deck.**
#[test]
fn test_20_covadd_errorr_covr_viewr_whole_deck() {
    assert_identical("20", whole_deck("20", "cl35rml", &[21, 22, 23, 91, 25, 26, 27]));
}
