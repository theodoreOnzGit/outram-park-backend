// SPDX-License-Identifier: AGPL-3.0-only
//! **GROUPR's flux calculator (`iwt < 0`, `nsigz > 1`), whole decks against
//! NJOY2016.80** (gh:#1370).
//!
//! # Methodology
//!
//! No upstream deck uses `iwt < 0` with several sigma-zeros, so four decks of our
//! own are run by the NJOY2016.80 binary (`NJOY2016_ROOT/build/njoy`) and by
//! [`run_deck`] on the same tape: Pu-238 ENDF/B-IV (`tests/resources/t404`,
//! MAT 1050; resolved 1-200 eV, unresolved 200 eV-10 keV). Each deck runs MODER,
//! RECONR (`err = 0.005`), BROADR and UNRESR at the GROUPR temperatures, then
//! GROUPR with LANL 30 groups, `iwt = -3`, `lord = 1`, `sigz = 1e10 100 10` and
//! card 9 `3 1/ 3 2/ 3 18/ 3 102/ 6 2/`, the GENDF on the text unit 26. Only
//! card 8a and the temperatures change:
//!
//! - `SD1`: 300 K, `150. 10. 2000 0 1/`: the homogeneous slowing-down flux below
//!   `fehi = 150` eV;
//! - `SD2`: 300 and 900 K, `150. 10. 2000 -30 2 0.7768 0.5 0.3 0.7143 0.4/`:
//!   three moderators, heterogeneity, and the `ninwt` file on unit 30 at
//!   `jsigz = 2` (compared byte for byte). `fehi` is carried to the second
//!   temperature as upstream's module variable is, but here the grid ends on
//!   `fehi` itself, so the carry is not tested;
//! - `SD3`: `SD1` with `ninwt = 30` at `jsigz = 3`;
//! - `SD0`: 300 K, `150. 10. 0 0 1/`: `nflmax = 0`, the Bondarenko branch with
//!   card 8a's `sigpot`.
//!
//! Pass criterion: unit 26 identical line for line, compared as strings (and
//! unit 30 byte for byte for `SD2` and `SD3`); the deck runs to its `stop`.
//!
//! # Prediction (posted on gh:#1370 before the first run)
//!
//! Unit 26 identical in all three decks (`SD0`-`SD2`); `SD2`'s unit 30 identical.
//! `SD3` was added after `SD2`'s first run, to separate the moderators from `ninwt`.
//!
//! # Results (2026-11-14, NJOY2016.80, release)
//!
//! - `SD0`, `SD1`, `SD3`: unit 26 identical (411 of 411 lines); `SD3`'s unit 30
//!   identical (122 064 bytes).
//! - `SD2`: unit 26 identical (820 lines); unit 30 identical (233 440 bytes)
//!   after one fix. The first run had 19 of 7 296 records 1 ulp apart: step 3's
//!   third-moderator term is `((f1*beta)*(1-gamma))*(sigz-sam)` upstream.
//! - Ablations: zero elastic in the solve, 63 of `SD1`'s 411 lines differ;
//!   `sigpot = 0` in `SD0`, 213 of 411.
//!
//! Record: deck-driver README §32; predictions and results on gh:#1370.

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

mod common;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// The Pu-238 deck at `temps` with GROUPR card 8a `card8a`.
fn deck(temps: &[f64], card8a: &str) -> String {
    let t: Vec<String> = temps.iter().map(|x| format!("{x:.1}")).collect();
    let t = t.join(" ");
    let n = temps.len();
    format!(
        "moder\n20 -21/\nreconr\n-21 -22/\n'pu238'/\n1050/\n0.005/\n0/\n\
         broadr\n-21 -22 -23/\n1050 {n} 0 1 0/\n0.005/\n{t}/\n0/\n\
         unresr\n-21 -23 -24/\n1050 {n} 3 1/\n{t}/\n1.e10 100. 10./\n0/\n\
         groupr\n-21 -24 0 26/\n1050 3 0 -3 1 {n} 3 1/\n'pu238 flux calculator'/\n{t}/\n1.e10 100. 10./\n\
         {card8a}\n3 1/\n3 2/\n3 18/\n3 102/\n6 2/\n0/\n{}0/\nstop\n",
        "3 1/\n3 2/\n3 18/\n3 102/\n6 2/\n0/\n".repeat(n - 1)
    )
}

/// Run `deck` in NJOY2016.80 and here on `t404`; compare text unit 26 line for
/// line and each unit of `binary` byte for byte. Returns the number of
/// differing lines of unit 26, or `None` (skipped).
fn differences(tag: &str, deck: &str, binary: &[u32]) -> Option<usize> {
    let root = njoy_root();
    let bin = root.join("build/njoy");
    let res = root.join("tests/resources/t404");
    if !bin.exists() || !res.exists() {
        eprintln!("skip: {} or {} not present (set NJOY2016_ROOT)", bin.display(), res.display());
        return None;
    }
    let tmp = std::env::temp_dir().join(format!("njoy_deck_fc_{tag}_{}", std::process::id()));
    let (ours, theirs) = (tmp.join("ours"), tmp.join("njoy"));
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::copy(&res, d.join("tape20")).unwrap();
    }
    std::fs::write(theirs.join("deck"), deck).unwrap();
    let (ok, date) = common::run_njoy_dated(&bin, &theirs);
    assert!(ok, "NJOY2016 binary failed on {tag}");
    let run = run_deck(deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date) }).unwrap();
    assert_eq!(run.blocked, None, "{tag} must run to its end");
    let read = |d: &Path| std::fs::read_to_string(d.join("tape26")).unwrap_or_default();
    let (a, b) = (read(&ours), read(&theirs));
    let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let d: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
    println!("{tag} unit 26: {} differing lines of {}", d.len(), bl.len());
    for &i in d.iter().take(6) {
        println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, al.get(i), bl.get(i));
    }
    assert!(!b.is_empty(), "{tag}: NJOY2016 wrote no unit 26");
    for u in binary {
        let rd = |d: &Path| std::fs::read(d.join(format!("tape{u}"))).unwrap_or_default();
        let (x, y) = (rd(&ours), rd(&theirs));
        let first = x.iter().zip(&y).position(|(p, q)| p != q);
        println!("{tag} unit {u}: {} bytes here, {} in NJOY; first differing byte {first:?}", x.len(), y.len());
        assert!(!y.is_empty(), "{tag}: NJOY2016 wrote no unit {u}");
        assert!(x == y, "{tag}: unit {u} differs from NJOY2016.80");
    }
    if std::env::var_os("KEEP_DECK_DIRS").is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    Some(d.len())
}

/// `SD1`: the homogeneous slowing-down flux (`nalph = 1`), 300 K.
#[test]
fn sd1_groupr_slowing_down_flux() {
    if let Some(n) = differences("sd1", &deck(&[300.0], "150. 10. 2000 0 1/"), &[]) {
        assert_eq!(n, 0, "sd1: unit 26 differs from NJOY2016.80");
    }
}

/// `SD2`: three moderators and heterogeneity, two temperatures, `ninwt` = 30.
#[test]
fn sd2_groupr_slowing_down_heterogeneous_ninwt_two_temperatures() {
    let d = deck(&[300.0, 900.0], "150. 10. 2000 -30 2 0.7768 0.5 0.3 0.7143 0.4/");
    if let Some(n) = differences("sd2", &d, &[30]) {
        assert_eq!(n, 0, "sd2: unit 26 differs from NJOY2016.80");
    }
}

/// `SD3`: `SD1` (one moderator) with the `ninwt` file at `jsigz = 3`.
#[test]
fn sd3_groupr_slowing_down_ninwt() {
    if let Some(n) = differences("sd3", &deck(&[300.0], "150. 10. 2000 -30 3/"), &[30]) {
        assert_eq!(n, 0, "sd3: unit 26 differs from NJOY2016.80");
    }
}

/// `SD0`: `nflmax = 0`, the Bondarenko branch with card 8a's `sigpot = 10`.
#[test]
fn sd0_groupr_bondarenko_with_sigpot() {
    if let Some(n) = differences("sd0", &deck(&[300.0], "150. 10. 0 0 1/"), &[]) {
        assert_eq!(n, 0, "sd0: unit 26 differs from NJOY2016.80");
    }
}
