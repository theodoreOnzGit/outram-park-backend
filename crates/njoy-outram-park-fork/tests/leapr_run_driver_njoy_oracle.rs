//! LEAPR `run_deck` driver against three NJOY2016 LEAPR runs — the whole
//! deck to the whole tape, MF=1/MT=451 header included.
//!
//! Oracles (`reference-data/leapr/*.njoy-input`, exactly the decks NJOY2016
//! `ac5adf5` read; the `.endf` next to each is its `tape24`):
//!
//! | run | what the driver must get right |
//! |---|---|
//! | H-in-H2O 293.6 K (MAT 1) | `contin`+`trans`+`discre`, free-gas secondary (`b7 = 1`), `iel = 0` with `twt > 0` -> **no** MT=2, 54 comment cards, dictionary of 2 |
//! | D-in-D2O 293.6 K (MAT 11) | the same chain plus the Sköld correction (`nsk = 2`) |
//! | Si in α-quartz (MAT 47) | **five temperatures** in one MF=7/MT=4 (`LT = 4`), the mixed-moderator second pass and merge, `iel = 0` with `twt = 0` -> **incoherent elastic** (`leapr.f90:3043`, `LTHR = 2`, `SB = sb*npr = 2.16877`), 71 comment cards, dictionary of 3 |
//!
//! # Predictions (stated before measuring)
//!
//! - MF=1/MT=451: every line (columns 1-75; sequence numbers are cosmetic)
//!   **byte-identical** to NJOY's — the HEAD/CONT floats go through the same
//!   `a11` formatter, the Hollerith cards are the deck's own text, and the
//!   dictionary `NC` values follow the Fortran's estimate formulas.
//! - MF=7: every data row of every section within **1e-5** relative of the
//!   oracle's row (both sides store 7-figure `sigfig` values, so a last-digit
//!   flip is the only allowed difference), same row count — which pins the
//!   `LT = ntempr-1` layout, the two `T_eff` TAB1s of the mixed moderator
//!   and the incoherent-elastic TAB1 at once.

use std::path::Path;

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::run::run_deck;
use njoy_outram_park_fork::leapr::vintage::PhysicalConstants;
use njoy_outram_park_fork::reference_data::reference_file_or_skip;

const LABEL: &str = "leapr-run-driver";
const ROW_TOL: f64 = 1e-5;

struct Case {
    name: &'static str,
    mat: i32,
    stem: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "H-in-H2O 293.6 K",
        mat: 1,
        stem: "tsl-HinH2O-293.6K-njoy2016-leapr",
    },
    Case {
        name: "SiO2-alpha (5 T, mixed, incoherent elastic)",
        mat: 47,
        stem: "tsl-SiO2-alpha-njoy2016-leapr",
    },
    Case {
        name: "D-in-D2O 293.6 K (skold)",
        mat: 11,
        stem: "tsl-DinD2O-293.6K-njoy2016-leapr",
    },
];

/// The deck text NJOY read: from the `leapr` line to the `stop` line of
/// the committed `.njoy-input` (which may wrap it in a shell heredoc).
fn deck_text(path: &Path) -> String {
    let text = std::fs::read_to_string(path).expect("njoy-input readable");
    let mut out = String::new();
    let mut inside = false;
    for line in text.lines() {
        if !inside {
            if line.trim() == "leapr" {
                inside = true;
                out.push_str("leapr\n");
            }
            continue;
        }
        if line.trim() == "EOF" {
            break;
        }
        out.push_str(line);
        out.push('\n');
        if line.trim() == "stop" {
            break;
        }
    }
    assert!(inside, "no `leapr` line in {}", path.display());
    out
}

/// The MF=1/MT=451 lines of a tape file, columns 1-75.
fn mf1_lines(text: &str, mat: i32) -> Vec<String> {
    text.lines()
        .filter(|l| l.len() >= 75 && &l[66..75] == format!("{mat:4} 1451"))
        .map(|l| l[..75].to_string())
        .collect()
}

fn run_case(case: &Case) {
    let tag = format!("[{LABEL}] {}", case.name);
    let Some(deck_path) =
        reference_file_or_skip("leapr", &format!("{}.njoy-input", case.stem), LABEL)
    else {
        return;
    };
    let Some(tape_path) = reference_file_or_skip("leapr", &format!("{}.endf", case.stem), LABEL)
    else {
        return;
    };
    let deck = LeaprDeck::parse(&deck_text(&deck_path))
        .expect("deck parses")
        .with_constants(PhysicalConstants::Codata2018);
    let t0 = std::time::Instant::now();
    let run = run_deck(&deck).unwrap_or_else(|e| panic!("{tag}: run_deck: {e}"));
    println!("{tag}: run_deck took {:.1} s", t0.elapsed().as_secs_f64());

    // --- MF=1/MT=451, byte for byte (columns 1-75) ---
    let njoy_text = std::fs::read_to_string(&tape_path).expect("oracle readable");
    let ours_text = run.write_text().expect("write_text");
    let want = mf1_lines(&njoy_text, case.mat);
    let got = mf1_lines(&ours_text, case.mat);
    assert_eq!(
        ours_text.lines().next().map(|l| &l[..75]),
        njoy_text.lines().next().map(|l| &l[..75]),
        "{tag}: TPID record"
    );
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        assert_eq!(g, w, "{tag}: MF=1 line {} differs", i + 1);
    }
    assert_eq!(got.len(), want.len(), "{tag}: MF=1 line count");
    println!("{tag}: MF=1/MT=451 {} lines identical", got.len());

    // --- MF=7: row by row ---
    let njoy = Tape::read_file(&tape_path).expect("oracle parses");
    let ours = Tape::read(ours_text.as_bytes()).expect("our tape parses");
    let njoy_mf7: Vec<_> = njoy
        .sections()
        .iter()
        .filter(|s| s.key.mat == case.mat && s.key.mf == 7)
        .collect();
    let ours_mf7: Vec<_> = ours
        .sections()
        .iter()
        .filter(|s| s.key.mat == case.mat && s.key.mf == 7)
        .collect();
    assert_eq!(
        ours_mf7.iter().map(|s| s.key.mt).collect::<Vec<_>>(),
        njoy_mf7.iter().map(|s| s.key.mt).collect::<Vec<_>>(),
        "{tag}: MF=7 sections"
    );
    for (o, n) in ours_mf7.iter().zip(&njoy_mf7) {
        assert_eq!(
            o.rows.len(),
            n.rows.len(),
            "{tag}: MF=7/MT={} row count",
            n.key.mt
        );
        let mut worst = (0.0f64, 0usize, 0usize);
        let mut over = 0usize;
        for (r, (a, b)) in o.rows.iter().zip(&n.rows).enumerate() {
            for f in 0..6 {
                let scale = a[f].abs().max(b[f].abs());
                let rel = if scale > 0.0 {
                    (a[f] - b[f]).abs() / scale
                } else {
                    0.0
                };
                if rel > worst.0 {
                    worst = (rel, r + 1, f + 1);
                }
                if rel > ROW_TOL {
                    over += 1;
                }
            }
        }
        println!(
            "{tag}: MF=7/MT={} {} rows, worst {:.3e} at row {} field {}, {} fields over {ROW_TOL:.0e}",
            n.key.mt, n.rows.len(), worst.0, worst.1, worst.2, over
        );
        assert_eq!(over, 0, "{tag}: MF=7/MT={} fields over tolerance", n.key.mt);
    }
    // the dictionary's MF=7 entries name the sections that were written
    let dict: Vec<(i32, i32)> = run
        .header
        .dictionary()
        .into_iter()
        .map(|(mf, mt, _, _)| (mf, mt))
        .collect();
    let mut expected = vec![(1, 451)];
    expected.extend(ours_mf7.iter().map(|s| (7, s.key.mt)));
    assert_eq!(dict, expected, "{tag}: dictionary vs sections written");
}

#[test]
fn h2o_run_matches_njoy_tape() {
    run_case(&CASES[0]);
}

#[test]
fn sio2_five_temperature_mixed_run_matches_njoy_tape() {
    run_case(&CASES[1]);
}

#[test]
fn d2o_skold_run_matches_njoy_tape() {
    run_case(&CASES[2]);
}
