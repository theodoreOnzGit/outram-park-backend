// SPDX-License-Identifier: AGPL-3.0-only
//! **Photo-nuclear decks against NJOY2016.80** (gh:#1369): RECONR on a
//! photo-nuclear tape, ACER `iopt = 5` (`acephn`) and ACER `iopt = 7` on its
//! table (`phnfix`).
//!
//! # Methodology
//!
//! Upstream decks 57 (Bi-209 ENDF/B-VIII.0), 58 (Co-59 IAEA), 66 (Pu-239 IAEA)
//! and 78 (He-3 IAEA) are run **as written** by the NJOY2016.80 binary
//! (`NJOY2016_ROOT/build/njoy`) and by [`run_deck`], each on the same
//! `tests/resources` tape mounted as unit 20. Compared line by line: RECONR's
//! PENDF through MODER (unit 90; deck 57 has no MODER copy), the `iopt = 5`
//! ACE table and directory (31, 32), and the first `iopt = 7` table and
//! directory (34, 35). Pass criterion: zero differing lines on every unit.
//! Our run must stop at the plot file of the first `iopt = 7` step, by name
//! (`phnplo` is not ported), and at nothing earlier. A case skips with a
//! message when the NJOY clone is absent.
//!
//! # Predictions (posted on gh:#1369 before each change)
//!
//! 1. RECONR: tape90 identical on 58, 66, 78 after the MF=2/MT=151 stub for a
//!    tape without MF=2 and MT=1 always summed and labelled MT=3 for `AWI = 0`.
//! 2. `acephn` (rewritten as a line-for-line translation): 31 and 32 identical
//!    on all four.
//! 3. `phnfix`: 34 and 35 identical on all four.
//!
//! # Results (2026-11-13, NJOY2016.80 `ac5adf5`, gfortran 13.3, release)
//!
//! All three held on the first run. Lines compared: deck 78 57/3 488/1/3 488/1,
//! 57 -/57 803/1/57 803/1, 66 124/259 515/1/259 515/1, 58 4 668/132 970/1/132 970/1
//! (units 90/31/32/34/35). Deck 56 (U-235) stops at the MF=4 refusal and is not
//! gated here.

use std::path::{Path, PathBuf};
use std::process::Command;

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// Run upstream deck `case` in NJOY2016.80 and in this crate, `resource` on
/// unit 20; compare `units` line by line. `None`: skipped.
fn photonuclear_deck(case: &str, resource: &str, units: &[u32]) -> Option<Vec<(u32, usize, usize)>> {
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
    let deck = std::fs::read_to_string(&input).unwrap();
    let tmp = std::env::temp_dir().join(format!("njoy_deck_phn{case}_{}", std::process::id()));
    let (ours, theirs) = (tmp.join("ours"), tmp.join("njoy"));
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::copy(&res, d.join("tape20")).unwrap();
    }
    std::fs::write(theirs.join("deck"), &deck).unwrap();
    let st = Command::new("sh")
        .arg("-c")
        .arg(format!("{} < deck > log 2>&1", bin.display()))
        .current_dir(&theirs)
        .status()
        .unwrap();
    assert!(st.success(), "NJOY2016 binary failed on deck {case}");
    let run = run_deck(&deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false }).unwrap();
    let blocked = run.blocked.map(|(m, why)| format!("{m}: {why}")).unwrap_or_default();
    assert!(blocked.contains("phnplo"), "deck {case} must stop at the phnplo refusal and nothing earlier, got {blocked:?}");
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

fn assert_identical(case: &str, r: Option<Vec<(u32, usize, usize)>>) {
    let Some(r) = r else { return };
    for (u, d, n) in r {
        assert!(n > 0 && d == 0, "deck {case} unit {u}: {d} of {n} lines differ");
    }
}

/// **Deck 78 (He-3 IAEA):** `LCT = 2`, Kalbach-Mann neutron and proton,
/// deuteron LAW=2 `LANG = 12`; no MF=2 and no MT=1 on the tape.
#[test]
fn test_78_photonuclear_pendf_ace_and_check_table() {
    assert_identical("78", photonuclear_deck("78", "g-002-He-3-IAEA.endf", &[90, 31, 32, 34, 35]));
}

/// **Deck 57 (Bi-209 ENDF/B-VIII.0):** `LCT = 3`, five Kalbach particles, 30
/// LAW=1 recoils feeding the heating.
#[test]
fn test_57_photonuclear_ace_and_check_table() {
    assert_identical("57", photonuclear_deck("57", "g-083_Bi_209-ENDF8.0.endf", &[31, 32, 34, 35]));
}

/// **Deck 66 (Pu-239 IAEA):** the MF=1 nubar replaces MT=18's neutron yield,
/// `LANG = 1` with `NA` up to 10 (`ptleg2`), LAW=0 recoils.
#[test]
fn test_66_photonuclear_pendf_ace_and_check_table() {
    assert_identical("66", photonuclear_deck("66", "g-094-Pu-239-IAEA.endf", &[90, 31, 32, 34, 35]));
}

/// **Deck 58 (Co-59 IAEA):** MT=1 on the tape relabelled MT=3, LAW=2
/// `LANG = 0` on about 100 MTs, MT=102 capture heating, MT=600-639/800-839.
#[test]
fn test_58_photonuclear_pendf_ace_and_check_table() {
    assert_identical("58", photonuclear_deck("58", "g-027_Co_59-IAEA.endf", &[90, 31, 32, 34, 35]));
}
