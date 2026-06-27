//! Integration tests: parse the He-4 ENDF/B-VIII.0 evaluation.
//!
//! This is the canonical Phase-1 fixture — 109 lines, one material (MAT 228),
//! three sections (MF=1/MT=451, MF=3/MT=2, MF=6/MT=2). Verified against the
//! file distributed with NJOY2016 at `tests/resources/a-002_He_004-ENDF8.0.endf`.
//!
//! Run with:
//! ```bash
//! cargo test -p njoy-outram-park-fork --release -- --include-ignored
//! ```

use std::fs::File;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::interp::eval_tab1;

fn he4_path() -> std::path::PathBuf {
    // The fixture lives in the crate's tests/resources/ directory.
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/resources/a-002_He_004-ENDF8.0.endf");
    p
}

#[test]
fn tape_parses_without_error() {
    let f = File::open(he4_path()).expect("fixture not found");
    let tape = Tape::read(f).expect("parse failed");
    // 3 data sections: MF=1/MT=451, MF=3/MT=2, MF=6/MT=2
    assert_eq!(tape.len(), 3, "expected 3 sections, got {}", tape.len());
}

#[test]
fn section_keys_are_correct() {
    let tape = Tape::read(File::open(he4_path()).unwrap()).unwrap();
    let keys: Vec<_> = tape.sections().iter().map(|s| (s.key.mat, s.key.mf, s.key.mt)).collect();
    assert!(keys.contains(&(228, 1, 451)), "missing MF=1 MT=451");
    assert!(keys.contains(&(228, 3,   2)), "missing MF=3 MT=2");
    assert!(keys.contains(&(228, 6,   2)), "missing MF=6 MT=2");
}

#[test]
fn mf3_mt2_cont_header() {
    // MF=3 MT=2: elastic cross section CONT record
    // First line: ZA=2004 (He-4), AWR=3.968219, LR=0, ...
    let tape = Tape::read(File::open(he4_path()).unwrap()).unwrap();
    let sec = tape.section(228, 3, 2).expect("MF=3 MT=2 not found");
    let mut cur = SectionCursor::new(&sec.rows);
    let cont = cur.read_cont().unwrap();
    assert!((cont.c1 - 2004.0).abs() < 0.01, "ZA={}", cont.c1);
    assert!((cont.c2 - 3.968219).abs() < 1e-5, "AWR={}", cont.c2);
}

#[test]
fn mf3_mt2_tab1_point_count() {
    // MF=3 MT=2: TAB1 cross section — should have 29 (E, σ) pairs
    let tape = Tape::read(File::open(he4_path()).unwrap()).unwrap();
    let sec = tape.section(228, 3, 2).expect("MF=3 MT=2 not found");
    let mut cur = SectionCursor::new(&sec.rows);
    let _cont = cur.read_cont().unwrap();  // ZA/AWR CONT
    let tab1 = cur.read_tab1().unwrap();
    assert_eq!(tab1.pairs.len(), 29, "expected 29 cross-section points");
    assert_eq!(tab1.head.n1, 1, "NR (interp regions) should be 1");
    assert_eq!(tab1.interp[0].1, 2, "interpolation law should be lin-lin (2)");
}

#[test]
fn mf3_mt2_first_and_last_energy() {
    // First energy point: 2.95e5 eV; last: 2.0e7 eV
    let tape = Tape::read(File::open(he4_path()).unwrap()).unwrap();
    let sec = tape.section(228, 3, 2).unwrap();
    let mut cur = SectionCursor::new(&sec.rows);
    let _cont = cur.read_cont().unwrap();
    let tab1 = cur.read_tab1().unwrap();
    let (e_first, _) = tab1.pairs[0];
    let (e_last, _)  = tab1.pairs[tab1.pairs.len() - 1];
    assert!((e_first - 2.95e5).abs() < 1.0, "first E = {}", e_first);
    assert!((e_last  - 2.0e7 ).abs() < 1.0, "last E  = {}", e_last);
}

#[test]
fn mf3_mt2_cross_section_at_known_point() {
    // At E=2.95e5 eV, σ=0.0 b (from the ENDF file: 0.000000+0)
    let tape = Tape::read(File::open(he4_path()).unwrap()).unwrap();
    let sec = tape.section(228, 3, 2).unwrap();
    let mut cur = SectionCursor::new(&sec.rows);
    let _cont = cur.read_cont().unwrap();
    let tab1 = cur.read_tab1().unwrap();
    let (_, sigma_first) = tab1.pairs[0];
    assert_eq!(sigma_first, 0.0, "σ(2.95e5 eV) should be 0");

    // At E=2.0e7 eV, σ≈0.1529574 b
    let (_, sigma_last) = tab1.pairs[28];
    assert!((sigma_last - 0.1529574).abs() < 1e-6, "σ(20 MeV) = {}", sigma_last);
}

#[test]
fn interpolation_within_he4_tab1() {
    // Interpolate at an intermediate energy using eval_tab1
    let tape = Tape::read(File::open(he4_path()).unwrap()).unwrap();
    let sec = tape.section(228, 3, 2).unwrap();
    let mut cur = SectionCursor::new(&sec.rows);
    let _cont = cur.read_cont().unwrap();
    let tab1 = cur.read_tab1().unwrap();

    // The table uses law 2 (lin-lin) throughout.
    // Interpolate at E=1.0e6 eV, which is between the tabulated points;
    // the result should be between the two surrounding σ values.
    let e_target = 1.0e6_f64;
    let sigma = eval_tab1(e_target, &tab1.interp, &tab1.pairs).unwrap();
    // From the file: 1.097400+6 → 2.317137-2; 5.958800+5 → 1.179131-2
    // lin-lin at 1e6 is between these
    assert!(sigma > 0.0, "σ > 0 at 1 MeV");
    assert!(sigma < 0.05, "σ < 0.05 b at 1 MeV (rough bound)");
}
