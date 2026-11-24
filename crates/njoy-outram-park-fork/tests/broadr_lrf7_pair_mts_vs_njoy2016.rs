// Reads reference files or the deck work directory: needs `std` (gh:#1592).
#![cfg(feature = "std")]

// SPDX-License-Identifier: AGPL-3.0-only
//! **BROADR's LRF=7 particle-pair list `nppmt`** (gh:#618), against NJOY2016.
//!
//! # Methodology
//!
//! Upstream BROADR reads the MT of every LRF=7 particle pair from the ENDF
//! tape's MF=2/MT=151 (`broadr.f90:303-330`) and broadens each one whatever
//! its threshold (`:513-520`), ahead of the `enext <= emin` and
//! `lrp = 1, enext < eresh` tests. The list only matters for a pair MT whose
//! threshold is at or above `eresh`.
//!
//! **No held tape has one** (checked 2026-11-24 over every LRF=7 evaluation in
//! `reference-data/endf/` and NJOY2016's `tests/resources/`): Cl-35 VII.1
//! (pairs 2, 102, 600), Ca-40 (2, 102, 600, 800; MT=600 from 542 keV, `eresh`
//! 1.5 MeV), Fe-54 (2, 102), **Fe-57** (2, 51, 102; MT=51 from 14.668 keV,
//! `eresh` 190 keV), Cu-63/65 (2, 102), Sr-88 (2, 102) and Mo-95 (2, 102, 800,
//! 801) all have every pair channel open below `eresh`.
//!
//! So the case is a **modified** tape: Fe-57 ENDF/B-VIII.0 (MAT 2634) with the
//! resolved range's `EH` on MF=2's range record lowered from 1.9e5 to 1.0e4 eV,
//! which puts MT=51's 14.668 keV threshold above `eresh`. The modification is
//! made in the test from `reference-data/endf/n-026_Fe_057-ENDF8.0.endf`; it
//! makes no physical claim (elastic is zero from 10 keV to 190 keV) and is fed
//! to both codes. Deck: `reconr 20 21 / 2634 0 / .001 / 0 /`, then
//! `broadr 20 21 22 / 2634 1 / .001 / 293.6 / 0 /`, all text units. NJOY2016
//! (`build/njoy`, else `build2/njoy`, under `NJOY2016_ROOT` or
//! `upstream_source/NJOY2016`) and this crate's deck driver run it; the test
//! skips when either the binary or the tape is absent.
//!
//! **Pass criterion:** the BROADR tape (unit 22) identical line for line.
//!
//! # Results (2026-11-24)
//!
//! Predicted before the run (gh:#618): MF=3 values below `thnmax` unchanged
//! (MT=51 is zero there, so the thinning and MT=1 do not move); the forced
//! MT=51 sets `mt4br`, so MT=4 is rebuilt on the broadened grid instead of
//! copied, and differs.
//!
//! NJOY2016 lists `mt 2 5 51 102 107 800 801 802` as broadened (2 433 points
//! in, 2 107 out). With the list: **0 differing lines** on units 21 and 22.
//! Ablated (the deck driver given an empty list, as the port was before):
//! **60 differing lines, all MF=3 MT=4** (NJOY's MT=4 is rebuilt; the port
//! copied it). That ablation was a one-off build, not kept in the code.

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;

mod common;

fn njoy_binary() -> Option<PathBuf> {
    let root = std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"));
    ["build/njoy", "build2/njoy"].iter().map(|b| root.join(b)).find(|p| p.exists())
}

fn lines(p: &Path) -> Vec<String> {
    std::fs::read_to_string(p).unwrap().lines().map(str::to_string).collect()
}

/// Fe-57 with MF=2's first range `EH` (columns 12-22 of its third MF=2 line)
/// replaced by `eh`, an ENDF 11-column float.
fn fe57_with_eh(text: &str, eh: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut mf2_line = 0;
    for line in text.lines() {
        if line.len() >= 75 && &line[66..75] == "2634 2151" {
            mf2_line += 1;
            if mf2_line == 3 {
                assert_eq!(&line[11..22], " 1.900000+5", "Fe-57's resolved range ends at 190 keV");
                out.push_str(&line[..11]);
                out.push_str(eh);
                out.push_str(&line[22..]);
                out.push('\n');
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[test]
fn fe57_pair_channel_above_eresh_is_broadened_as_njoy2016_does() {
    let Some(bin) = njoy_binary() else {
        eprintln!("skip: NJOY2016 binary not present (set NJOY2016_ROOT)");
        return;
    };
    let Some(endf) = reference_endf_or_skip("n-026_Fe_057-ENDF8.0.endf", "Fe-57 ENDF/B-VIII.0") else { return };
    let tape20 = fe57_with_eh(&std::fs::read_to_string(endf).unwrap(), " 1.000000+4");
    let deck = "reconr\n20 21\n'fe57 eh 10 kev'/\n2634 0/\n.001/\n0/\n\
                broadr\n20 21 22\n2634 1/\n.001/\n293.6/\n0/\nstop\n";

    let tmp = std::env::temp_dir().join(format!("njoy_nppmt_{}", std::process::id()));
    let (ours, theirs) = (tmp.join("ours"), tmp.join("njoy"));
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::write(d.join("tape20"), &tape20).unwrap();
    }
    std::fs::write(theirs.join("deck"), deck).unwrap();
    let (ok, date) = common::run_njoy_dated(&bin, &theirs);
    assert!(ok, "NJOY2016 failed on the Fe-57 deck");
    let log = std::fs::read_to_string(theirs.join("output")).unwrap_or_default();
    assert!(log.contains("mt   2   5  51 102"), "NJOY2016 broadens the pair MT=51: {log}");
    let run = run_deck(deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date) }).unwrap();
    assert_eq!(run.blocked, None);

    let mut bad = Vec::new();
    for u in [21, 22] {
        let (a, b) = (lines(&ours.join(format!("tape{u}"))), lines(&theirs.join(format!("tape{u}"))));
        let d = (0..a.len().max(b.len())).filter(|&i| a.get(i) != b.get(i)).count();
        println!("Fe-57 (EH 10 keV) unit {u}: {d} differing lines of {}", b.len());
        if d > 0 {
            bad.push((u, d));
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(bad.is_empty(), "units differ from NJOY2016: {bad:?}");
}

/// `lrf7_pair_mts` on the held LRF=7 tapes reads the lists named in the module
/// doc (and none from a non-LRF=7 evaluation).
#[test]
fn pair_lists_of_the_held_lrf7_tapes() {
    use njoy_outram_park_fork::broadr::joint::lrf7_pair_mts;
    use njoy_outram_park_fork::endf::tape::Tape;
    for (file, mat, want) in [
        ("n-017_Cl_035-ENDF7.1.endf", 1725, vec![2, 102, 600]),
        ("n-026_Fe_057-ENDF8.0.endf", 2634, vec![2, 51, 102]),
        ("n-020_Ca_040-ENDF8.0.endf", 2025, vec![2, 102, 600, 800]),
        ("n-014_Si_030-ENDF8.0.endf", 1431, vec![]),
    ] {
        let Some(p) = reference_endf_or_skip(file, file) else { continue };
        let tape = Tape::read_file(&p).expect("ENDF tape");
        assert_eq!(lrf7_pair_mts(&tape, mat), want, "{file}");
    }
}
