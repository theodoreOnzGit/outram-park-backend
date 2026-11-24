// Reads reference files or the deck work directory: needs `std` (gh:#1592).
#![cfg(feature = "std")]

// SPDX-License-Identifier: AGPL-3.0-only
//! **RECONR on a multi-isotope MF=2 (`NIS > 1`), a resolved-unresolved overlap,
//! upstream's one `lssf`, and an unresolved-only `LSSF = 1` material, against
//! NJOY2016.80** (GitHub #1324, #666).
//!
//! # Methodology
//!
//! No evaluation held in `reference-data/endf/` or in NJOY2016's
//! `tests/resources/` reaches these branches: every MF=2 there has `NIS = 1`, no
//! resolved-unresolved overlap, and its unresolved range read last (scanned
//! 2026-11-24, 70 + 59 tapes, including the multi-material ENDF/B-IV and -V
//! tapes 404 and 511). So each case is Ar-37 (TENDL-2023, MAT 1828: an MLBW
//! range to 4 268.035 eV, then an `LRF = 2`, `LSSF = 1` unresolved range to
//! 5 548.45 eV) with its MF=2/MT=151 edited in this test:
//!
//! - **`nis2`**: `NIS = 2`. Isotope 1 is Ar-37's own MF=2 with `ABN = 0.7`;
//!   isotope 2 (`ZAI = 18036`, `ABN = 0.3`) is one SLBW range on
//!   `[1e-5, 3000]` eV with three s-wave resonances (150, 900, 2100 eV). The
//!   two isotopes' resolved ranges overlap from 1e-5 to 3 000 eV, where
//!   `sigma` sums both (`reconr.f90:2595-2646`). The SLBW range is read last,
//!   so upstream's `lssf` is 0 (`rdf2bw`, `:926`) although Ar-37's unresolved
//!   range says `LSSF = 1`: `genunr` adds the MF=3 background, and `resxs`
//!   walks the unresolved range.
//! - **`nis2_urr`**: as `nis2`, and isotope 2 also has a Case A unresolved range
//!   (`LRF = 1`, `LFW = 0`, `LSSF = 0`) on `[3000, 5000]` eV, read last. So
//!   `genunr` sums two isotopes' unresolved sections (`:1670-1680`), and
//!   `eresu = 3000 < eresr = 4268.035`: a resolved-unresolved overlap, with the
//!   `eunr` energies below `eresr` negated (`:856-866`).
//! - **`overlap`**: `NIS = 1`, Ar-37 with its MLBW range raised to
//!   `eh = 5000` eV and its unresolved range set to `LSSF = 0`. The overlap is
//!   `[4268.035, 5000)` eV, where `sigma` adds `sigunr` to the resolved sum
//!   (`:2660-2666`) and `emerge` keeps the MF=3 background.
//! - **`urr_only`**: Ar-37's unresolved range alone (`LSSF = 1`), so
//!   `eresl = eresu` and `resxs` returns at once (`nrtot = 0`, `:2275-2279`):
//!   `emerge` treats no reaction as a resonance reaction.
//!
//! These are code-to-code devices: the edited tapes have no physical meaning.
//! NJOY2016.80 (`build/njoy`) and [`run_deck`] run `reconr 20 22` (tolerance
//! 0.001) on the same text. **Pass criterion:** tape 22 identical line for line,
//! MF=1 to MF=3 including MF=2/MT=152.
//!
//! # Prediction (posted on gh:#1324 and gh:#666 before the first run)
//!
//! Tape 22 identical on all four. Before the change `nis2` and `nis2_urr` were
//! refused (`NotPorted`), and `overlap` would have lost the resolved part on
//! `[4268.035, 5000)` eV.
//!
//! # Results (2026-11-24, NJOY2016.80 `build2/njoy`, gfortran, release)
//!
//! See the table in the doc comment of [`reconr_multi_isotope_and_overlap_match_njoy2016`].

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};
use njoy_outram_park_fork::endf::tape::{Section, Tape};
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::NjoyError;

mod common;

const MAT: i32 = 1828;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

fn ar37() -> Option<Tape> {
    let p = njoy_outram_park_fork::reference_data::reference_endf("n-018_Ar_37-tendl2023.endf")?;
    Some(Tape::read_file(&p).expect("Ar-37 tape"))
}

/// `tape` with its MF=2/MT=151 rows replaced by `rows`.
fn with_mf2(tape: &Tape, rows: Vec<[f64; 6]>) -> Tape {
    let mut secs: Vec<Section> = tape.sections().to_vec();
    let sec = secs.iter_mut().find(|s| s.key.mat == MAT && s.key.mf == 2 && s.key.mt == 151).expect("MF=2/MT=151");
    sec.rows = rows;
    Tape::from_sections(String::from("synthetic RECONR multi-isotope / overlap tape"), secs)
}

fn mf2_rows(tape: &Tape) -> Vec<[f64; 6]> {
    tape.section(MAT, 2, 151).expect("MF=2").rows.clone()
}

/// Index of Ar-37's unresolved range CONT `(4268.035, 5548.45, 2, 2, 0, 0)`.
fn urr_cont(rows: &[[f64; 6]]) -> usize {
    rows.iter().position(|r| r[2] == 2.0 && r[3] == 2.0 && (r[1] - 5548.45).abs() < 1e-6).expect("LRU=2 range CONT")
}

/// Isotope 2: an SLBW range on `[1e-5, 3000]` eV, and optionally a Case A
/// unresolved range on `[3000, 5000]` eV with `LSSF = 0`.
fn isotope2(with_urr: bool) -> Vec<[f64; 6]> {
    let mut rows = vec![
        [18036.0, 0.3, 0.0, 0.0, if with_urr { 2.0 } else { 1.0 }, 0.0],
        [1.0e-5, 3000.0, 1.0, 1.0, 0.0, 0.0],
        [0.0, 0.5, 0.0, 0.0, 1.0, 0.0],
        [35.67, 0.0, 0.0, 0.0, 18.0, 3.0],
        [150.0, 0.5, 1.2, 1.0, 0.2, 0.0],
        [900.0, 0.5, 3.3, 3.0, 0.3, 0.0],
        [2100.0, 0.5, 5.25, 5.0, 0.25, 0.0],
    ];
    if with_urr {
        rows.extend([
            [3000.0, 5000.0, 2.0, 1.0, 0.0, 0.0],
            [0.0, 0.5, 0.0, 0.0, 1.0, 0.0],
            [35.67, 0.0, 0.0, 0.0, 6.0, 1.0],
            [300.0, 0.5, 1.0, 0.03, 0.25, 0.0],
        ]);
    }
    rows
}

fn nis2(base: &[[f64; 6]], with_urr: bool) -> Vec<[f64; 6]> {
    let mut rows = base.to_vec();
    rows[0][4] = 2.0; // NIS
    rows[1][1] = 0.7; // ABN of isotope 1
    rows.extend(isotope2(with_urr));
    rows
}

fn overlap(base: &[[f64; 6]]) -> Vec<[f64; 6]> {
    let mut rows = base.to_vec();
    assert_eq!(rows[2][2], 1.0, "row 2 is the MLBW range CONT");
    rows[2][1] = 5000.0; // resolved EH above the unresolved EL
    let k = urr_cont(&rows);
    rows[k + 1][2] = 0.0; // LSSF
    rows
}

fn urr_only(base: &[[f64; 6]]) -> Vec<[f64; 6]> {
    let k = urr_cont(base);
    let mut rows = vec![base[0], base[1]];
    rows[1][4] = 1.0; // NER
    rows.extend_from_slice(&base[k..]);
    rows
}

const DECK: &str = "reconr\n20 22/\n'synthetic'/\n1828 0/\n0.001/\n0/\nstop\n";

fn lines(p: &Path) -> Vec<String> {
    std::fs::read_to_string(p).unwrap_or_default().lines().map(str::to_string).collect()
}

/// `reconr 20 22` on `text` with NJOY2016 and the port; `(ours, NJOY's)` tape 22.
fn run_both(root: &Path, tmp: &Path, name: &str, text: &str) -> (Vec<String>, Vec<String>) {
    let (ours, theirs) = (tmp.join(format!("{name}_ours")), tmp.join(format!("{name}_njoy")));
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::write(d.join("tape20"), text).unwrap();
    }
    std::fs::write(theirs.join("deck"), DECK).unwrap();
    let (ok, date) = common::run_njoy_dated(&root.join("build/njoy"), &theirs);
    assert!(ok, "NJOY2016 failed on {name}");
    let run = run_deck(DECK, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date) }).unwrap();
    assert_eq!(run.blocked, None, "{name} must run to its end");
    (lines(&ours.join("tape22")), lines(&theirs.join("tape22")))
}

/// MF=2/MT=152 and MF=3 line counts of a PENDF.
fn count(v: &[String], mf: &str) -> usize {
    v.iter().filter(|l| l.len() >= 75 && &l[70..72] == mf).count()
}

/// **The four cases against NJOY2016.80, tape 22 line for line.**
///
/// Results (2026-11-24; see the module doc for the method):
///
/// | case | tape 22 lines | MF=2 lines (151 + 152) | MF=3 lines | differing |
/// |---|---|---|---|---|
/// | `nis2` | 9 449 | 15 | 9 168 | **0** |
/// | `nis2_urr` | 9 350 | 21 | 9 063 | **0** |
/// | `overlap` | 7 964 | 15 | 7 683 | **0** |
/// | `urr_only` | 5 970 | 15 | 5 689 | **0** |
///
/// NJOY prints "mat has resolved-unresolved overlap" for `nis2_urr` and
/// `overlap`, and `nis2_urr`'s MT=152 carries the negated energies
/// `-3.000001+3 … -4.268034+3`.
///
/// What the first runs found (each fixed in the port, then re-run):
/// - `nis2`: 1 line, MT=152's `INTUNR` (2 against NJOY's 5). `rdfil2` resets
///   `intunr = 5` before every range, so the SLBW range read last decides it;
///   the port took the last Case C range's.
/// - `nis2_urr` 8 521 and `overlap` 7 410 lines: the walk was split at
///   `eresu`, restarting the step-increase state below `eresr`. `resxs` accepts
///   the crossing panel untested and carries `in` and `res(1)` on, so NJOY
///   bisected the 4 268.035 eV resonance's flank more finely (4 268.041 eV
///   against 4 268.055 eV). The crossing is now an untested panel inside one
///   stretch (`RebuildOpts::untested_crossings`).
#[test]
fn reconr_multi_isotope_and_overlap_match_njoy2016() {
    let root = njoy_root();
    if !root.join("build/njoy").exists() {
        eprintln!("skip: {} not present (set NJOY2016_ROOT)", root.join("build/njoy").display());
        return;
    }
    let Some(ar) = ar37() else { return };
    let base = mf2_rows(&ar);
    let tmp = std::env::temp_dir().join(format!("njoy_reconr_nis_{}", std::process::id()));
    let mut bad = Vec::new();
    for (name, rows) in [
        ("nis2", nis2(&base, false)),
        ("nis2_urr", nis2(&base, true)),
        ("overlap", overlap(&base)),
        ("urr_only", urr_only(&base)),
    ] {
        let text = with_mf2(&ar, rows).to_text().unwrap();
        let (a, b) = run_both(&root, &tmp, name, &text);
        let d: Vec<usize> = (0..a.len().max(b.len())).filter(|&i| a.get(i) != b.get(i)).collect();
        println!(
            "{name}: {} differing lines of {} (NJOY: {} MF=2 lines, {} MF=3 lines; ours {} lines)",
            d.len(),
            b.len(),
            count(&b, " 2"),
            count(&b, " 3"),
            a.len()
        );
        for &i in d.iter().take(8) {
            println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, a.get(i), b.get(i));
        }
        assert!(count(&b, " 3") > 100, "{name}: NJOY's tape 22 must carry MF=3");
        if !d.is_empty() {
            bad.push((name, d.len()));
        }
    }
    if std::env::var_os("KEEP_DECK_DIRS").is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    assert!(bad.is_empty(), "tape 22 differs from NJOY2016: {bad:?}");
}

/// GitHub #666 item 3: an SLBW/MLBW l-state with `LRX != 0` (a competitive
/// width) is refused, not reconstructed without its MT=51 channel and its width
/// in the line shapes (`reconr.f90:964-1016, 2808, 2839, 2852`).
#[test]
fn lrx_competitive_width_is_refused() {
    let Some(ar) = ar37() else { return };
    let mut rows = mf2_rows(&ar);
    // Row 4 is the first l-state LIST head (AWRI, QX, L, LRX, 6*NRS, NRS).
    assert_eq!(rows[4][4], 30.0, "the l=0 LIST head");
    rows[4][3] = 1.0;
    let tape = with_mf2(&ar, rows);
    match reconr(&tape, &ReconrConfig { mat: MAT, tolerance: 0.001, temperature: 0.0 }) {
        Err(NjoyError::NotPorted(m)) => assert!(m.contains("LRX"), "{m}"),
        other => panic!("LRX = 1 must be refused, got {:?}", other.map(|_| ())),
    }
}
