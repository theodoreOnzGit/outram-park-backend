// SPDX-License-Identifier: AGPL-3.0-only
//! **Photo-nuclear decks against NJOY2016.80, whole decks** (gh:#1369):
//! RECONR on a photo-nuclear tape, ACER `iopt = 5` (`acephn`), the two ACER
//! `iopt = 7` check passes (`phnfix` + `phnplo`) and VIEWR on their plot files.
//!
//! # Methodology
//!
//! Upstream decks 57 (Bi-209 ENDF/B-VIII.0), 58 (Co-59 IAEA), 66 (Pu-239 IAEA)
//! and 78 (He-3 IAEA) are run **whole, as written** by the NJOY2016.80 binary
//! (`NJOY2016_ROOT/build/njoy`) and by [`run_deck`], each on the same
//! `tests/resources` tape mounted as unit 20. Every **text** tape NJOY leaves is
//! compared line by line: RECONR's PENDF through MODER (90), the `iopt = 5` ACE
//! table and directory (31, 32), each check pass's plot file, table and
//! directory (33/34/35 and 36/37/38), and VIEWR's PostScript (43, 46). Not
//! compared: the unchanged input tape20 and MODER's blocked-binary unit 30,
//! which this crate writes as text. Pass criterion: zero differing lines on
//! every compared tape, every unit in the deck's list present, and the run
//! reaching the deck's `stop` (no refusal). A case skips with a message when
//! the NJOY clone is absent.
//!
//! # Predictions (posted on gh:#1369 before each change)
//!
//! 1. RECONR: tape90 identical on 58, 66, 78 after the MF=2/MT=151 stub for a
//!    tape without MF=2 and MT=1 always summed and labelled MT=3 for `AWI = 0`.
//! 2. `acephn` (rewritten as a line-for-line translation): 31 and 32 identical
//!    on all four.
//! 3. `phnfix`: 34 and 35 identical on all four.
//! 4. `phnplo` (`acer::phnplo`): unit 33 identical on all four, and with it the
//!    second check pass and VIEWR, so all four decks identical whole.
//!
//! # Results (2026-11-13, NJOY2016.80 `ac5adf5`, gfortran 13.3, release)
//!
//! Predictions 1-3 held on the first run. Lines compared, units
//! 90/31/32/34/35: deck 78 57/3 488/1/3 488/1, 57 -/57 803/1/57 803/1,
//! 66 124/259 515/1/259 515/1, 58 4 668/132 970/1/132 970/1. Prediction 4: see
//! the results comment on gh:#1369 and the deck-driver README §22 (unit 33:
//! 5 637, 59 125, 33 524 and 59 951 lines). Deck 56 (U-235) stops at the MF=4
//! refusal and is not gated here.

use std::path::PathBuf;

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

mod common;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

fn is_text(b: &[u8]) -> bool {
    !b.is_empty() && b.iter().take(4096).all(|&c| c == b'\n' || c == b'\r' || c == b'\t' || (32..127).contains(&c))
}

/// Run upstream deck `case` whole in NJOY2016.80 and in this crate, `resource`
/// on unit 20; compare every text tape NJOY wrote line by line. `expect` lists
/// the units that must be among them. `None`: skipped.
fn photonuclear_deck(case: &str, resource: &str, expect: &[u32]) -> Option<Vec<(String, usize, usize)>> {
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
    let (ok, date) = common::run_njoy_dated(&bin, &theirs); // gh:#1549
    assert!(ok, "NJOY2016 binary failed on deck {case}");
    let run = run_deck(&deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date.clone()) }).unwrap();
    assert!(run.blocked.is_none(), "deck {case} must run to its end, stopped at {:?}", run.blocked);
    let staged = std::fs::read(&res).unwrap();
    let mut names: Vec<String> = std::fs::read_dir(&theirs)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("tape"))
        .collect();
    names.sort();
    let mut out = Vec::new();
    for t in names {
        let b = std::fs::read(theirs.join(&t)).unwrap();
        if (t == "tape20" && b == staged) || !is_text(&b) {
            continue;
        }
        let a = std::fs::read(ours.join(&t)).unwrap_or_default();
        let (a, b) = (String::from_utf8_lossy(&a).to_string(), String::from_utf8_lossy(&b).to_string());
        let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
        let diff: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
        println!("deck {case} {t}: {} differing lines of {}", diff.len(), bl.len());
        for &i in diff.iter().take(4) {
            println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, al.get(i), bl.get(i));
        }
        out.push((t, diff.len(), bl.len()));
    }
    for u in expect {
        assert!(out.iter().any(|(t, _, _)| *t == format!("tape{u}")), "deck {case}: NJOY wrote no text tape{u}");
    }
    if std::env::var_os("DECK_KEEP").is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    Some(out)
}

fn assert_identical(case: &str, r: Option<Vec<(String, usize, usize)>>) {
    let Some(r) = r else { return };
    for (t, d, n) in r {
        assert!(n > 0 && d == 0, "deck {case} {t}: {d} of {n} lines differ");
    }
}

/// **Deck 78 (He-3 IAEA):** `LCT = 2`, Kalbach-Mann neutron and proton,
/// deuteron LAW=2 `LANG = 12`; no MF=2 and no MT=1 on the tape. One check pass.
#[test]
fn test_78_photonuclear_whole_deck() {
    assert_identical("78", photonuclear_deck("78", "g-002-He-3-IAEA.endf", &[90, 31, 32, 33, 34, 35, 43]));
}

/// **Deck 57 (Bi-209 ENDF/B-VIII.0):** `LCT = 3`, five Kalbach particles, 30
/// LAW=1 recoils feeding the heating.
#[test]
fn test_57_photonuclear_whole_deck() {
    assert_identical("57", photonuclear_deck("57", "g-083_Bi_209-ENDF8.0.endf", &[31, 32, 33, 34, 35, 36, 37, 38, 43, 46]));
}

/// **Deck 66 (Pu-239 IAEA):** the MF=1 nubar replaces MT=18's neutron yield,
/// `LANG = 1` with `NA` up to 10 (`ptleg2`), LAW=0 recoils.
#[test]
fn test_66_photonuclear_whole_deck() {
    assert_identical("66", photonuclear_deck("66", "g-094-Pu-239-IAEA.endf", &[90, 31, 32, 33, 34, 35, 36, 37, 38, 43, 46]));
}

/// **Deck 58 (Co-59 IAEA):** MT=1 on the tape relabelled MT=3, LAW=2
/// `LANG = 0` on about 100 MTs, MT=102 capture heating, MT=600-639/800-839.
#[test]
fn test_58_photonuclear_whole_deck() {
    assert_identical("58", photonuclear_deck("58", "g-027_Co_59-IAEA.endf", &[90, 31, 32, 33, 34, 35, 36, 37, 38, 43, 46]));
}

/// **Deck 56 (U-235 ENDF/B-VIII.0):** MT=18's neutrons in MF=4 (isotropic) and
/// MF=5 (`LF = 7`, Maxwellian), the prompt nubar from MF=1/MT=456; MT=5 and
/// MT=16 in MF=6 (`acer::photonuclear::mf45`).
#[test]
fn test_56_photonuclear_whole_deck() {
    assert_identical("56", photonuclear_deck("56", "g-092_U_235-ENDF8.0.endf", &[31, 32, 33, 34, 35, 36, 37, 38, 43, 46]));
}

/// **Deck 64 (Ra-226 TENDL-2019, gh:#1369):** MF=6 recoils given as `LAW = 4`,
/// whose heating `acephn` takes from the section's first (two-body)
/// subsection, `awp*(e+q)/(awr-awp)` (`acepn.f90:507-584`), and an MT=11 photon
/// yield that is zero everywhere. Upstream's first-positive-yield scan
/// (`:417-424`) has no bound there; this crate takes the all-zero yield as
/// giving no production threshold (a divergence on undefined behaviour,
/// commented in `photonuclear/build.rs`). Prediction (posted on gh:#1369
/// before the run): units 31-38, 43 and 46 identical; the `LAW = 4` refusal
/// met next was not foreseen. **Result (2026-11-13): every unit identical.**
#[test]
fn test_64_photonuclear_whole_deck() {
    assert_identical("64", photonuclear_deck("64", "g-088_Ra226-tendl2019.endf", &[31, 32, 33, 34, 35, 36, 37, 38, 43, 46]));
}
