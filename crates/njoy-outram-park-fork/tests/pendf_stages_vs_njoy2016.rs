//! RECONR and BROADR against NJOY2016's own PENDFs, **word for word**
//! (GitHub #340).
//!
//! # Methodology
//!
//! For each committed golden, this crate runs the same deck and compares
//! every MF=3 section of the result with NJOY's, after both have been through
//! PENDF text (`ReconrResult::through_pendf_text`, 11-column ENDF floats).
//!
//! - **RECONR** decks: `reconr / <mat> 0 / .001 /`. Our side is
//!   `reconr::reconr` at `tolerance = 0.001`.
//! - **BROADR** decks: add `broadr / <mat> 1 / .001 / 293.6 /`. Our side is
//!   `reconr` followed by `broadr::broaden_result`, upstream's joint
//!   multi-reaction walk (`broadr::joint`).
//!
//! **Pass criterion:** every section NJOY writes is present in ours, and
//! each has the same QI and the same `(E, sigma)` pairs, bit for bit. Ours
//! must also carry no section NJOY lacks. There is no tolerance.
//!
//! References are NJOY2016 `ac5adf5f` outputs committed under
//! `reference-data/reconr/` (`tape21`) and `reference-data/errorr/`
//! (`tape22`). Each directory's README gives the deck and provenance.
//!
//! # Results, 2026-09-26
//!
//! All 12 cases below are **identical in every word**. The row counts are the
//! MF=3 points per section of the union grid:
//!
//! | case | stage | points (MT=1) | formalism exercised |
//! |---|---|---|---|
//! | Si-30 ENDF/B-VIII.0 | RECONR | 7 284 | RM (LRF=3) |
//! | Sr-88 ENDF/B-VIII.1 | RECONR | — | RML (LRF=7) with background R-matrix |
//! | Ar-37 TENDL-2023 | RECONR | 1 786 | MLBW + URR, MF=10 in the union |
//! | U-234 ENDF/B-VIII.0 | RECONR | 45 977 | RM + URR (LSSF=0), MT=4 redundant |
//! | H-2, Li-6, Be-9, C-12, F-19 ENDF/B-VIII.0 | BROADR 293.6 K | — | LRP=0 `thnmax` |
//! | Si-30 ENDF/B-VIII.0 | BROADR 293.6 K | — | RM |
//! | Cl-35 ENDF/B-VII.1 | BROADR 293.6 K | — | RML (LRF=7) |
//! | Ar-37 TENDL-2023 (L1 last) | BROADR 293.6 K | — | MLBW + URR |
//!
//! Earlier on 2026-09-26, what was measured before the fixes was this: the
//! U-234 ACE grid shared 5.5 % of NJOY's energies; U-234 RECONR lacked the
//! 1e-5 to 0.0253 eV range and differed across the unresolved range; Sr-88,
//! Ar-37, Li-6 and C-12 each differed as the fixes below record. The other
//! cases were not measured word for word before this test. The fixes, in the
//! order found, are in the crate's
//! `verification_and_validation/ace_block_parity/ace_block_parity_2026_09_26.md`.
//! U-234 and U-238 at 293.6 K, NJOY's own RECONR/BROADR tapes, are
//! compared by `examples/pendf_stage_vs_njoy.rs`. They are not committed:
//! U-238's pair is 138 MB.

use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::reference_file_or_skip;

/// Compare ours with NJOY's PENDF; returns one line per differing section.
fn differences(endf: &str, mat: i32, pendf_dir: &str, pendf: &str, temp_k: Option<f64>) -> Option<Vec<String>> {
    let tape_path = reference_file_or_skip("endf", endf, endf)?;
    let pendf_path = reference_file_or_skip(pendf_dir, pendf, pendf)?;
    let tape = Tape::read_file(&tape_path).expect("ENDF tape");
    let theirs = Tape::read_file(&pendf_path).expect("NJOY PENDF");
    let r0 = reconr(
        &tape,
        &ReconrConfig {
            mat,
            tolerance: 1.0e-3,
            temperature: 0.0,
        },
    )
    .expect("RECONR");
    let ours = match temp_k {
        Some(t) => broaden_result(&r0, t),
        None => r0,
    }
    .through_pendf_text();

    let mut diffs = Vec::new();
    let mut their_mts = Vec::new();
    for sec in theirs.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 3) {
        let mt = sec.key.mt;
        their_mts.push(mt);
        let mut cur = SectionCursor::new(&sec.rows);
        let _ = cur.read_cont().expect("HEAD");
        let t = cur.read_tab1().expect("TAB1");
        match ours.sections.iter().find(|s| s.mt.number() == mt) {
            None => diffs.push(format!("MT={mt}: missing in ours")),
            Some(s) if s.pairs != t.pairs || s.qi != t.head.c2 => {
                let first = s.pairs.iter().zip(&t.pairs).position(|(a, b)| a != b);
                diffs.push(format!(
                    "MT={mt}: ours {} points, NJOY {}; first differing pair {first:?}; QI {} vs {}",
                    s.pairs.len(),
                    t.pairs.len(),
                    s.qi,
                    t.head.c2
                ));
            }
            Some(_) => {}
        }
    }
    for s in &ours.sections {
        if !their_mts.contains(&s.mt.number()) {
            diffs.push(format!("MT={}: extra in ours", s.mt.number()));
        }
    }
    Some(diffs)
}

fn assert_same(endf: &str, mat: i32, dir: &str, pendf: &str, temp_k: Option<f64>) {
    let Some(d) = differences(endf, mat, dir, pendf, temp_k) else {
        return;
    };
    assert!(d.is_empty(), "{pendf}: {} sections differ:\n{}", d.len(), d.join("\n"));
    println!("{pendf}: every MF=3 word identical to NJOY2016's");
}

#[test]
fn reconr_si30_rm() {
    assert_same("n-014_Si_030-ENDF8.0.endf", 1431, "reconr", "si30-ENDF8.0-0K.pendf", None);
}

#[test]
fn reconr_sr88_rml_background_rmatrix() {
    assert_same("n-038_Sr_088-ENDF8.1.endf", 3837, "reconr", "sr88-ENDF8.1-0K-err0.001.pendf", None);
}

#[test]
fn reconr_ar37_mlbw_urr_and_mf10() {
    assert_same("n-018_Ar_37-tendl2023.endf", 1828, "reconr", "ar37-tendl2023-0K.pendf", None);
}

#[test]
fn reconr_u234_rm_urr() {
    assert_same("n-092_U_234-ENDF8.0.endf", 9225, "reconr", "u234-ENDF8.0-0K-err0.001.pendf", None);
}

#[test]
fn broadr_light_nuclides_lrp0() {
    for (endf, mat, pendf) in [
        ("n-001_H_002-ENDF8.0.endf", 128, "h2-ENDF8.0-293.6K.pendf"),
        ("n-003_Li_006-ENDF8.0.endf", 325, "li6-ENDF8.0-293.6K.pendf"),
        ("n-004_Be_009-ENDF8.0.endf", 425, "be9-ENDF8.0-293.6K.pendf"),
        ("n-006_C_012-ENDF8.0.endf", 625, "c12-ENDF8.0-293.6K.pendf"),
        ("n-009_F_019-ENDF8.0.endf", 925, "f19-ENDF8.0-293.6K.pendf"),
    ] {
        assert_same(endf, mat, "errorr", pendf, Some(293.6));
    }
}

#[test]
fn broadr_si30_rm() {
    assert_same("n-014_Si_030-ENDF8.0.endf", 1431, "errorr", "si30-ENDF8.0-293.6K.pendf", Some(293.6));
}

#[test]
fn broadr_cl35_rml() {
    assert_same("n-017_Cl_035-ENDF7.1.endf", 1725, "errorr", "cl35-ENDF7.1-293.6K.pendf", Some(293.6));
}

#[test]
fn broadr_ar37_mlbw_urr() {
    assert_same(
        "n-018_Ar_37-tendl2023-mf2-L1-last.endf",
        1828,
        "errorr",
        "ar37-tendl2023-L1last-293.6K.pendf",
        Some(293.6),
    );
}
