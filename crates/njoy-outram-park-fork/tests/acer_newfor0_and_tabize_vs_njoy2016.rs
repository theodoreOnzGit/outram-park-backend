// Reads reference files and runs decks in a work directory: needs `std` (gh:#1592).
#![cfg(feature = "std")]

// SPDX-License-Identifier: AGPL-3.0-only
//! **ACER, two branches no upstream test deck reaches, against NJOY2016.80**
//! (gh:#1372, 2026-11-24).
//!
//! # Methodology
//!
//! Each case runs the same deck in two work directories: the NJOY2016.80 binary
//! (`$NJOY2016_ROOT/build/njoy`, `ab27c64`) and [`run_deck`], on the same tape as
//! unit 20, this crate's run given NJOY's date. Every line of the ACE file (unit
//! 24) and the directory file (unit 25) is compared as a string. Pass criterion:
//! 0 differing lines. Skips with a message when the binary or the F-19 tape
//! (`reference-data/endf/n-009_F_019-ENDF8.0.endf`) is absent.
//!
//! **F-19, `newfor = 0`** (`F19_NEWFOR0`): MODER to binary, RECONR at 0.005,
//! BROADR to 293.6 K, ACER `iopt = 1` with card 6 `0 1 1` (old angular format).
//! MT=16's first neutron subsection (MF=6 LAW=1, `LANG = 1`) goes through
//! `topfil`'s `fix6` to LAW=7 with 33 cosines, and `pttab` reduces each incident
//! energy's cosine table to 32 equally probable bins for AND.
//!
//! **`tabize`'s linearise branch** (`tabize_*`): no held evaluation has an MF=1
//! nubar with a range of `INT > 2`, so the F-19 tape is given a synthetic
//! MF=1/MT=452 (`LNU = 2`, 6 points, ranges `INT` 2, 3, 4, 5; dictionary
//! entry added), built by [`f19_with_synthetic_nubar`] in this file. It is
//! input to both codes alike and claims no physics. `first`'s `tabize`
//! (`acefc.f90:813-1059`) linearises it to one lin-lin range before `acelod`
//! writes the NU block; the port does it in `acer::tabize::tabize_tape`. Run on
//! binary units (`mscr < 0`) and on text units (`mscr > 0`, where the linearised
//! points are read back at `a11`'s precision).
//!
//! # Predictions (written before the runs)
//!
//! - F-19 `newfor = 0`: the 5 AND lines that differed (MT=16, 13 MeV, the 6th
//!   figure) are identical once `pttab` keeps `slpe` across bins, as upstream
//!   does; every other line is unchanged, so 0 differing lines.
//! - `tabize`, both unit modes: the NU block is NJOY's in every word (`NR = 0`,
//!   the linearised grid), so 0 differing lines. Before the port, it would have
//!   written the evaluation's four ranges and 6 points.
//!
//! # Results (2026-11-24, release, NJOY2016.80 `ab27c64`)
//!
//! - F-19 `newfor = 0`: 0 differing lines of 49 285 (unit 24) and the directory
//!   line identical. Before the `pttab` fix, 5 lines (20 words of MT=16's AND at
//!   13 MeV, each one unit in the 6th printed figure, e.g. `1.27950E-01` against
//!   NJOY's `1.27949E-01`). Cause: the port reset `pttab`'s `slpe` to 0 on every
//!   bin; upstream's is a routine-level local that the `aneed < 1e-4` branch
//!   leaves stale, so `p(k)` moves by the previous bin's slope there. A port
//!   defect, not round-off.
//! - `tabize`: 0 differing lines of 41 950 (unit 24) and the directory line, on
//!   binary units and on text units. NJOY2016's NU block is `LNU = 2`, `NR = 0`,
//!   `NE = 10` (the evaluation's 6 points and 4 ranges, linearised), so the
//!   branch is reached; the port before `acer::tabize` copied the ranges
//!   (`NR = 4`, `NE = 6`, `acer::nu`'s non-lin-lin path).

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};
use njoy_outram_park_fork::endf::parse::format_endf_float;

mod common;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// F-19, binary units, old angular format (card 6 `newfor = 0`).
const F19_NEWFOR0: &str = "moder\n20 -21\nreconr\n-21 -22\n'pendf for F-19'/\n925 0/\n0.005/\n0/\n\
broadr\n-21 -22 -23\n925 1 0 0 0./\n0.005/\n293.6/\n0/\n\
acer\n-21 -23 0 24 25\n1 0 1 .10/\n'F-19 newfor=0'/\n925 293.6/\n0 1 1/\n/\nstop\n";

/// The synthetic-nubar tape on binary units (`mscr < 0`).
const TABIZE_BINARY: &str = "moder\n20 -21\nreconr\n-21 -22\n'pendf'/\n925 0/\n0.005/\n0/\n\
broadr\n-21 -22 -23\n925 1 0 0 0./\n0.005/\n293.6/\n0/\n\
acer\n-21 -23 0 24 25\n1 0 1 .10/\n'synthetic nubar, tabize'/\n925 293.6/\n/\n/\nstop\n";

/// The synthetic-nubar tape on text units (`mscr > 0`).
const TABIZE_TEXT: &str = "reconr\n20 22\n'pendf'/\n925 0/\n0.005/\n0/\n\
broadr\n20 22 23\n925 1 0 0 0./\n0.005/\n293.6/\n0/\n\
acer\n20 23 0 24 25\n1 0 1 .10/\n'synthetic nubar, tabize'/\n925 293.6/\n/\n/\nstop\n";

/// One ENDF text line: six 11-column fields, then MAT/MF/MT and a sequence number.
fn endf_line(fields: &[String; 6], mf: i32, mt: i32, seq: usize) -> String {
    format!("{}{:4}{:2}{:3}{:5}\n", fields.concat(), 925, mf, mt, seq)
}

/// F-19 ENDF/B-VIII.0 with a synthetic MF=1/MT=452 inserted after MT=451, and
/// its dictionary entry (NXC + 1). The TAB1 has four ranges, `INT` 2, 3, 4, 5,
/// over 6 points from 1e-5 eV to 20 MeV (`INT = 3`, `4` and `5` are what
/// `tabize` linearises; `INT = 1` it would copy).
fn f19_with_synthetic_nubar(endf: &str) -> String {
    let f = |x: f64| format_endf_float(x);
    let i = |n: i64| format!("{n:>11}");
    let blank = || " ".repeat(11);
    let e = [1.0e-5, 1.0e3, 1.0e5, 1.0e6, 5.0e6, 2.0e7];
    let nu = [2.40, 2.41, 2.45, 2.55, 3.10, 5.00];
    let mut sec = String::new();
    sec.push_str(&endf_line(&[f(9019.0), f(18.835), i(0), i(2), i(0), i(0)], 1, 452, 1));
    sec.push_str(&endf_line(&[f(0.0), f(0.0), i(0), i(0), i(4), i(6)], 1, 452, 2));
    sec.push_str(&endf_line(&[i(2), i(2), i(3), i(3), i(4), i(4)], 1, 452, 3));
    sec.push_str(&endf_line(&[i(6), i(5), blank(), blank(), blank(), blank()], 1, 452, 4));
    let words: Vec<String> = e.iter().zip(nu.iter()).flat_map(|(&a, &b)| [f(a), f(b)]).collect();
    for (k, c) in words.chunks(6).enumerate() {
        let mut row: [String; 6] = std::array::from_fn(|_| blank());
        for (j, w) in c.iter().enumerate() {
            row[j] = w.clone();
        }
        sec.push_str(&endf_line(&row, 1, 452, 5 + k));
    }
    sec.push_str(&format!("{}{:4}{:2}{:3}{:5}\n", " ".repeat(66), 925, 1, 0, 99999));
    let mut out = String::new();
    let mut in451 = 0usize;
    let mut dict_done = false;
    for line in endf.lines() {
        let tag = line.get(66..75).unwrap_or("");
        if tag == " 925 1451" {
            in451 += 1;
            if in451 == 4 {
                // NXC (N2 of the fourth record) gains the new entry.
                let nxc: i64 = line[55..66].trim().parse().unwrap();
                out.push_str(&format!("{}{:>11}{}\n", &line[..55], nxc + 1, &line[66..]));
                continue;
            }
            out.push_str(line);
            out.push('\n');
            if !dict_done && line[22..44] == format!("{:>11}{:>11}", 1, 451) {
                out.push_str(&format!("{}{:>11}{:>11}{:>11}{:>11} 925 1451{:5}\n", " ".repeat(22), 1, 452, 6, 0, 0));
                dict_done = true;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
        if tag == " 925 1  0" && in451 > 0 && !sec.is_empty() {
            out.push_str(&sec);
            sec.clear();
        }
    }
    assert!(dict_done && sec.is_empty(), "synthetic nubar: MF=1/MT=451 layout not as expected");
    out
}

/// Run `deck` on `tape20` text in NJOY2016 and in this crate; the differing
/// line numbers (1-based) of units 24 and 25, or `None` (skip).
fn ace_differences(tag: &str, tape20: &str, deck: &str) -> Option<Vec<(u32, Vec<usize>)>> {
    let root = njoy_root();
    let bin = root.join("build/njoy");
    if !bin.exists() {
        eprintln!("skip: {} not present (set NJOY2016_ROOT)", bin.display());
        return None;
    }
    let tmp = std::env::temp_dir().join(format!("njoy_acer_{tag}_{}", std::process::id()));
    let (ours, theirs) = (tmp.join("ours"), tmp.join("njoy"));
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::write(d.join("tape20"), tape20).unwrap();
    }
    std::fs::write(theirs.join("deck"), deck).unwrap();
    let (ok, date) = common::run_njoy_dated(&bin, &theirs);
    assert!(ok, "NJOY2016 binary failed on {tag}");
    let run = run_deck(deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date) }).unwrap();
    assert_eq!(run.blocked, None, "{tag} must run to its end");
    let mut bad = Vec::new();
    for u in [24u32, 25] {
        let read = |d: &Path| std::fs::read_to_string(d.join(format!("tape{u}"))).unwrap_or_default();
        let (a, b) = (read(&ours), read(&theirs));
        assert!(!b.is_empty(), "{tag}: NJOY2016 wrote no unit {u}");
        let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
        let d: Vec<usize> = (0..al.len().max(bl.len())).filter(|&i| al.get(i) != bl.get(i)).map(|i| i + 1).collect();
        println!("{tag} unit {u}: {} differing lines of {}; first {:?}", d.len(), bl.len(), &d[..d.len().min(5)]);
        for &l in d.iter().take(3) {
            println!("  ours: {:?}\n  njoy: {:?}", al.get(l - 1), bl.get(l - 1));
        }
        if !d.is_empty() {
            bad.push((u, d));
        }
    }
    if std::env::var_os("KEEP_DECK_DIRS").is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    Some(bad)
}

fn f19_text() -> Option<String> {
    let Some(p) = njoy_outram_park_fork::reference_data::reference_endf_or_skip("n-009_F_019-ENDF8.0.endf", "F-19") else { return None };
    Some(std::fs::read_to_string(p).unwrap())
}

/// F-19 with `newfor = 0`: every line of the ACE and directory files is NJOY2016's.
#[test]
fn f19_newfor0_ace_is_njoy2016s() {
    let Some(t) = f19_text() else { return };
    let Some(bad) = ace_differences("f19nf0", &t, F19_NEWFOR0) else { return };
    assert!(bad.is_empty(), "F-19 newfor = 0: lines differ from NJOY2016: {bad:?}");
}

/// `tabize`'s linearise branch on binary units: the NU block is NJOY2016's.
#[test]
fn tabize_linearises_a_log_nubar_as_njoy2016_binary_units() {
    let Some(t) = f19_text() else { return };
    let Some(bad) = ace_differences("tabizeb", &f19_with_synthetic_nubar(&t), TABIZE_BINARY) else { return };
    assert!(bad.is_empty(), "tabize, binary units: lines differ from NJOY2016: {bad:?}");
}

/// `tabize`'s linearise branch on text units (`a11` read-back): NJOY2016's too.
#[test]
fn tabize_linearises_a_log_nubar_as_njoy2016_text_units() {
    let Some(t) = f19_text() else { return };
    let Some(bad) = ace_differences("tabizet", &f19_with_synthetic_nubar(&t), TABIZE_TEXT) else { return };
    assert!(bad.is_empty(), "tabize, text units: lines differ from NJOY2016: {bad:?}");
}
