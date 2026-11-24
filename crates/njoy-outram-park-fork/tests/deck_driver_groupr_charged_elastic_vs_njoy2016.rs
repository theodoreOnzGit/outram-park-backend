// Reads reference files or the deck work directory: needs `std` (gh:#1592).
#![cfg(feature = "std")]

// SPDX-License-Identifier: AGPL-3.0-only
//! **GROUPR's elastic cross section for an incident charged particle, with
//! `getdis`'s Coulomb term, whole decks against NJOY2016.80** (gh:#1370,
//! 2026-11-24).
//!
//! # Methodology
//!
//! No upstream test deck runs GROUPR on an incident charged particle, so these
//! decks are ours. Each runs MODER, RECONR (`err = 0.001`) and GROUPR at 0 K on one
//! ENDF/B-VIII.0 tape from NJOY2016's `tests/resources`: user groups (`ign = 1`),
//! `iwt = 3`, `lord = 1`, one sigma-zero, card 9 `3 2/`. The NJOY2016.80 binary
//! (`NJOY2016_ROOT/build/njoy`) and [`run_deck`] read the same tape, and unit 24
//! (text GENDF) is compared line for line, as strings. The run's date is passed
//! through so the header lines agree.
//!
//! What upstream does there: `conver` takes `izap = NSUB/10` and `awrp = AWI` from
//! MF=1; `getsig` divides the MF=3 AWR by `awrp` and gives 1 on a `1.1 e` grid;
//! `getff` goes to `getmf6` (`zad = izap`), which sends the projectile's `LAW = 5`
//! subsection to `getdis` with `mft = 8`. `getdis` reads the records through
//! `getfle` and `getco` (label 360 for `LTP = 1`, the amplitude expansion with the
//! Coulomb phase, its coefficients rounded to single precision by `cmplx`), adds
//! the Coulomb cross section, cuts the cosines at `wcut = 0.94` (and `-wcut` for
//! identical particles) and accumulates every band into one column.
//!
//! **Why two groups.** With more than two groups, upstream writes past the end of
//! its `ff` array: `getff` passes `ngn` to `getmf6` as `ng`, and `getmf6`'s normal
//! entry zeroes `ans(l, 1..ng)` (`groupr.f90:7965-7969`), while the main loop
//! allocated `ff(nlg, ng2g)` with `ng2g = 2` for `mfd = 3`. Valgrind on a scratch
//! NJOY2016 build (`-fcheck=bounds`, `-g`) reports the invalid write there, and the
//! runs abort with `free(): invalid pointer` on p + H-1, p + H-3, d + H-2, He-4 and
//! p + H-2 with six groups. With two groups there is no out-of-bounds write
//! (valgrind: 0 errors on every deck here), so the comparison is with upstream's
//! defined behaviour.
//!
//! Pass criterion: unit 24 identical line for line; the deck runs to its `stop`.
//! A case skips with a message when the NJOY clone or the tape is absent.
//!
//! # Predictions and results (2026-11-24, NJOY2016.80, release)
//!
//! - `C1` d + He-3 (MAT 225, `LTP = 1` and one `LTP = 0` record, `LIDP = 0`) and
//!   `C2` d + H-2 (MAT 128, `LTP = 1`, identical particles, `SPI = 1`): **run before
//!   a prediction was written**, while the code was being checked. Both were
//!   identical on that first run, 13 of 13 lines each; this test pins it.
//! - `C3` p + H-1 (MAT 125, `LTP = 1`, identical, `SPI = 1/2`), `C4` p + H-3 (MAT
//!   131, `LTP = 1`, `LIDP = 0`) and `C5` (p + H-1 again, groups 1e5-3e5-1e6 eV):
//!   predicted, in this comment before their first run, **identical line for
//!   line**. Result: identical, 13 of 13 lines each, on the first run.
//! - Below the printed figures (scratch NJOY2016 build printing every `getco`
//!   label-360 coefficient): 7 210 coefficients on the four tapes. Eight differ,
//!   all the unrounded `fl(1)`, by at most 1.4e-15 relative. With glibc's `cos`,
//!   `sin` and `ln` in place of PETIR's, all 7 210 are bit-identical, so the eight
//!   are the transcendentals' last bit, below the 1e-9 code-to-code allowance.
//! - Ablations: no Coulomb term, both cross-section lines differ in every deck;
//!   `cmplx` in double precision, 414 coefficients move at the 1e-8 level, though
//!   no printed value in these decks does.
//!
//! Record: deck-driver README §41.
//! - Refused by name: He-4 (MAT 228) and p + H-2, whose records are tabulated
//!   (`LTP = 12`); see `crate::groupr::charged_elastic`.

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};

mod common;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// The deck: MODER, RECONR, GROUPR `3 2/` on `mat` with the given boundaries.
fn deck(mat: i32, bounds: &str) -> String {
    let ng = bounds.split_whitespace().count() - 1;
    format!(
        "moder\n20 -21/\nreconr\n-21 -22/\n'cp'/\n{mat} 0/\n0.001/\n0/\n\
         groupr\n-21 -22 0 24/\n{mat} 1 0 3 1 1 1 1/\n'cp elastic'/\n0.0/\n1.0e10/\n\
         {ng}/\n{bounds}/\n3 2/\n0/\n0/\nstop\n"
    )
}

/// Run `deck` on the resource tape `name` in NJOY2016.80 and here; the number of
/// unit-24 lines differing, and NJOY's line count. `None`: skipped.
fn run_both(tag: &str, name: &str, deck: &str) -> Option<(usize, usize)> {
    run_both_mf(tag, name, deck, 3)
}

/// [`run_both`], checking that NJOY2016 wrote an `MF = mf`, `MT = 2` section.
fn run_both_mf(tag: &str, name: &str, deck: &str, mf: i32) -> Option<(usize, usize)> {
    let root = njoy_root();
    let bin = root.join("build/njoy");
    let tape = root.join("tests/resources").join(name);
    if !bin.exists() || !tape.exists() {
        eprintln!("skip: {} or {} not present (set NJOY2016_ROOT)", bin.display(), tape.display());
        return None;
    }
    let tmp = std::env::temp_dir().join(format!("njoy_deck_gce_{tag}_{}", std::process::id()));
    let (ours, theirs) = (tmp.join("ours"), tmp.join("njoy"));
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::copy(&tape, d.join("tape20")).unwrap();
    }
    std::fs::write(theirs.join("deck"), deck).unwrap();
    let (ok, date) = common::run_njoy_dated(&bin, &theirs);
    assert!(ok, "NJOY2016 binary failed on {tag}");
    let run = run_deck(deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date) }).unwrap();
    assert_eq!(run.blocked, None, "{tag} must run to its end");
    let read = |d: &Path| std::fs::read_to_string(d.join("tape24")).unwrap_or_default();
    let (a, b) = (read(&ours), read(&theirs));
    let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let diff: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).collect();
    println!("{tag} unit 24: {} differing lines of {}", diff.len(), bl.len());
    for &i in diff.iter().take(6) {
        println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, al.get(i), bl.get(i));
    }
    let key = format!("{mf:2}  2");
    assert!(bl.iter().any(|l| l.get(70..75) == Some(key.as_str())), "{tag}: NJOY2016 wrote no MF={mf}/MT=2");
    let _ = std::fs::remove_dir_all(&tmp);
    Some((diff.len(), bl.len()))
}

fn assert_identical(tag: &str, r: Option<(usize, usize)>) {
    if let Some((d, n)) = r {
        assert_eq!(d, 0, "{tag}: {d} of {n} lines differ from NJOY2016.80");
    }
}

const TWO_GROUPS: &str = "1.0e6 3.0e6 1.0e7";

/// `C1`: d + He-3 (`LIDP = 0`, `LTP = 1` and 0).
#[test]
fn c1_deuteron_on_he3() {
    assert_identical("c1", run_both("c1", "d-002_He_003.endf", &deck(225, TWO_GROUPS)));
}

/// `C2`: d + H-2 (identical particles, `SPI = 1`).
#[test]
fn c2_deuteron_on_h2() {
    assert_identical("c2", run_both("c2", "d-001_H_002-ENDF8.0.endf", &deck(128, TWO_GROUPS)));
}

/// `C3`: p + H-1 (identical particles, `SPI = 1/2`).
#[test]
fn c3_proton_on_h1() {
    assert_identical("c3", run_both("c3", "p-001_H_001-ENDF8.0.endf", &deck(125, TWO_GROUPS)));
}

/// `C4`: p + H-3 (`LIDP = 0`).
#[test]
fn c4_proton_on_h3() {
    assert_identical("c4", run_both("c4", "p-001_H_003-ENDF8.0.endf", &deck(131, TWO_GROUPS)));
}

/// `C5`: p + H-1 below 1 MeV.
#[test]
fn c5_proton_on_h1_low_energy() {
    assert_identical("c5", run_both("c5", "p-001_H_001-ENDF8.0.endf", &deck(125, "1.0e5 3.0e5 1.0e6")));
}

/// Tabulated records (`LTP = 12`, He-4 and p + H-2) are refused by name.
#[test]
fn tabulated_charged_elastic_is_refused() {
    for (tag, name, mat) in [("r1", "a-002_He_004-ENDF8.0.endf", 228), ("r2", "p-001_H_002-ENDF8.0.endf", 128)] {
        let tape = njoy_root().join("tests/resources").join(name);
        if !tape.exists() {
            eprintln!("skip: {} not present", tape.display());
            continue;
        }
        let dir = std::env::temp_dir().join(format!("njoy_deck_gce_{tag}_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(&tape, dir.join("tape20")).unwrap();
        let run = run_deck(&deck(mat, TWO_GROUPS), &DeckOptions { workdir: Some(dir.clone()), parse_only: false, run_date: None }).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let (_, msg) = run.blocked.expect("must be refused");
        assert!(msg.contains("LTP > 2"), "{tag}: {msg}");
    }
}

/// The matrix deck: GROUPR `mfd 2/` on `mat`, six groups 0.1-10 MeV, `lord = 1`.
fn matrix_deck(mat: i32, mfd: i32) -> String {
    format!(
        "moder\n20 -21/\nreconr\n-21 -22/\n'cp'/\n{mat} 0/\n0.001/\n0/\n\
         groupr\n-21 -22 0 24/\n{mat} 1 0 3 1 1 1 1/\n'cp elastic matrix'/\n0.0/\n1.0e10/\n\
         6/\n1.0e5 3.0e5 1.0e6 2.0e6 4.0e6 7.0e6 1.0e7/\n{mfd} 2/\n0/\n0/\nstop\n"
    )
}

/// `M1`-`M4`: the charged projectile's elastic **matrix**, card 9 `21 2/` (protons)
/// or `22 2/` (deuterons): `getff` label 800, `getmf6` with `zad` the projectile,
/// `getdis` with the Coulomb term and `wcut`, one band per sink group, written as
/// `MF = mfd` with `nl = 2`. Here upstream allocates `ff` for every group, so six
/// groups are used (valgrind on the scratch build: 0 errors on all four decks).
///
/// Prediction (written here before the port's first run on them, 2026-11-24):
/// unit 24 identical line for line on all four. NJOY2016 writes 27, 27, 27 and 26
/// lines.
///
/// Result (2026-11-24, NJOY2016.80): identical, 27, 27, 27 and 26 lines, on the
/// first run.
#[test]
fn m1_m4_charged_elastic_matrix() {
    for (tag, name, mat, mfd) in [
        ("m1", "d-002_He_003.endf", 225, 22),
        ("m2", "p-001_H_001-ENDF8.0.endf", 125, 21),
        ("m3", "d-001_H_002-ENDF8.0.endf", 128, 22),
        ("m4", "p-001_H_003-ENDF8.0.endf", 131, 21),
    ] {
        assert_identical(tag, run_both_mf(tag, name, &matrix_deck(mat, mfd), mfd));
    }
}
