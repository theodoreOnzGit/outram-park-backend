// SPDX-License-Identifier: AGPL-3.0-only
//! **GROUPR `getmf6` LAW = 2 for neutrons, against NJOY2016.80** (gh:#1370).
//!
//! # Methodology
//!
//! No upstream deck groups a File-6 `LAW = 2` neutron. The deck `L2` (ours) runs
//! Si-30 ENDF/B-VIII.0 (`reference-data/endf/n-014_Si_030-ENDF8.0.endf`, MAT 1431),
//! whose MF=6 MT 51-56 are `LAW = 2` with `LANG = 0` and `LCT = 2`, through:
//! - MODER, then RECONR at 0.005;
//! - BROADR to 293.6 K;
//! - GROUPR with LANL-30, `iwt = 3`, `lord = 3`, one sigma-zero, and card 9
//!   `3 1/ 3 51/ 8 51/` through `8 56/`, writing the GENDF as text on unit 25.
//!
//! NJOY2016.80 and [`run_deck`] run the same deck. Pass criterion: unit 25
//! identical line for line, compared as strings, and the deck runs to its `stop`.
//!
//! # Prediction (posted on gh:#1370 before the first run)
//!
//! Unit 25 identical.
//!
//! # Results (2026-11-14, NJOY2016.80, release)
//!
//! - Unit 25 identical, 332 of 332 lines, on the first run.
//! - Ablation: with the LAW = 2 records replaced by CM isotropy (the LAW = 3
//!   path), 181 of 332 lines differ.

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

mod common;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// The `L2` deck.
const L2: &str = "moder\n20 -21/\nreconr\n-21 -22/\n'si30'/\n1431/\n0.005/\n0/\n\
broadr\n-21 -22 -23/\n1431 1 0 0 0/\n0.001/\n293.6/\n0/\n\
groupr\n-21 -23 0 25/\n1431 3 0 3 3 1 1 1/\n'si30 law 2'/\n293.6/\n1.e10/\n\
3 1/\n3 51/\n8 51/\n8 52/\n8 53/\n8 54/\n8 55/\n8 56/\n0/\n0/\nstop\n";

/// **`L2`: File-6 `LAW = 2` neutrons grouped through `getdis`.**
#[test]
fn l2_groupr_mf6_law2_si30() {
    let bin = njoy_root().join("build/njoy");
    let Some(res) = njoy_outram_park_fork::reference_data::reference_endf("n-014_Si_030-ENDF8.0.endf") else {
        eprintln!("skip: Si-30 tape not present");
        return;
    };
    if !bin.exists() {
        eprintln!("skip: {} not present (set NJOY2016_ROOT)", bin.display());
        return;
    }
    let tmp = std::env::temp_dir().join(format!("njoy_deck_law2_{}", std::process::id()));
    let (ours, theirs) = (tmp.join("ours"), tmp.join("njoy"));
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::copy(&res, d.join("tape20")).unwrap();
    }
    std::fs::write(theirs.join("deck"), L2).unwrap();
    let (ok, date) = common::run_njoy_dated(&bin, &theirs);
    assert!(ok, "NJOY2016 binary failed on L2");
    let run = run_deck(L2, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date) }).unwrap();
    assert_eq!(run.blocked, None, "L2 must run to its end");
    let read = |d: &Path| std::fs::read_to_string(d.join("tape25")).unwrap_or_default();
    let (a, b) = (read(&ours), read(&theirs));
    let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let diff: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
    println!("L2 unit 25: {} differing lines of {}", diff.len(), bl.len());
    for &i in diff.iter().take(6) {
        println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, al.get(i), bl.get(i));
    }
    if std::env::var_os("KEEP_DECK_DIRS").is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    assert!(bl.len() > 100, "NJOY's unit 25 has only {} lines", bl.len());
    assert!(diff.is_empty(), "L2: {} of {} lines differ", diff.len(), bl.len());
}
