//! **The ACE comparator (`acer::compare`) finds what differs, and only that**
//! (GitHub #863).
//!
//! # Methodology
//!
//! The comparator is the instrument behind the claim that this port's ACE
//! tables are NJOY2016's word for word, and behind the nuclear data demo's
//! comparator rung. An instrument that cannot report a difference proves
//! nothing, so each test plants a known difference and checks it is found
//! where it was planted, and nowhere else.
//!
//! - **Continuous energy:** NJOY2016's own U-234 table at 293.6 K
//!   (`reference-data/ace`, 449 695 XSS words), read from its gzipped bytes
//!   the way the browser reads an uploaded file. Against itself it must be
//!   identical; with one SIG word changed, exactly that word, in SIG and in the
//!   whole XSS, and nothing else; with the NU block removed, NU missing.
//! - **Thermal S(α,β):** an Al-27 table this crate builds from the
//!   ENDF/B-VIII.0 tape, written as Type-1 text and read back, against itself
//!   and with one ITXE word changed.
//!
//! - **This port against NJOY2016, whole table:** U-234 at 293.6 K built by
//!   `build_deck` with NJOY's reference deck (RECONR 0.001, BROADR 293.6 K,
//!   PURR 20 bands / 64 ladders / 10 000 samples, no HEATR), written as Type-1
//!   text and read back, must be identical to NJOY2016's own table in the
//!   header, NXS, JXS and every XSS word. Until this date the whole-table
//!   result was checked only by running `examples/ace_blocks_vs_reference.rs`
//!   by hand.
//!
//! Pass criterion: the verdicts, counts and positions asserted below. There is
//! no tolerance: the comparator itself has none.
//!
//! # Results (2026-10-14)
//!
//! All pass. U-234 against itself: 25 CE block rows, each SAME or absent in
//! both, XSS SAME over 449 695 words. U-234 built here against NJOY2016's:
//! identical, material tag included, once `build_deck` wrote the MAT
//! (`   mat9225`) where it had written the ZA (`mat92234`), the one header
//! difference a browser build of the same deck exposed.

use njoy_outram_park_fork::acer::compare::{compare, Verdict, CE_BLOCKS, THERMAL_BLOCKS};
use njoy_outram_park_fork::acer::read::{self, AceFileType, RawAceTable};
use njoy_outram_park_fork::acer::thermal::{jxs as tjxs, ThermalAceOptions};
use njoy_outram_park_fork::acer::{build_deck, jxs, AceDeck, AceTable};
use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::{ace_reference_file_or_skip, reference_endf, reference_endf_dir};
use njoy_outram_park_fork::thermr::mf7::parse_mf7;

fn njoy_u234() -> Option<RawAceTable> {
    let p = ace_reference_file_or_skip("reference-njoy/endf-b-viii.0/293.6K/U234.ace.gz", "ace-compare")?;
    let bytes = std::fs::read(&p).expect("read the reference table");
    let mut t = read::read_library_from_bytes(bytes, "U234.ace.gz").expect("NJOY's U-234 table parses from bytes");
    assert_eq!(t.len(), 1);
    t.pop()
}

#[test]
fn a_table_is_identical_to_itself() {
    let Some(t) = njoy_u234() else { return };
    let c = compare(&t, &t);
    assert!(c.identical(), "{:#?}", c.report_lines());
    assert_eq!(c.blocks.len(), CE_BLOCKS.len());
    assert!(c.blocks.iter().all(|b| b.verdict.agrees()));
    assert_eq!(c.whole_xss.verdict, Verdict::Same);
    assert_eq!(c.whole_xss.words, (449_695, 449_695));
    let lines = c.report_lines();
    assert!(lines.last().unwrap().contains("identical"), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("SIG ") && l.contains("SAME")), "{lines:?}");
}

#[test]
fn one_changed_word_is_found_where_it_was_planted() {
    let Some(t) = njoy_u234() else { return };
    let mut u = t.clone();
    let sig = t.jxs[jxs::SIG] as usize; // 1-based start of SIG
    let at = sig + 10; // the 11th word of SIG
    u.xss[at - 1] *= 1.0 + 1e-6;
    let c = compare(&t, &u);
    assert!(!c.identical());
    for b in &c.blocks {
        if b.name == "SIG" {
            assert_eq!(b.verdict, Verdict::Different);
            assert_eq!(b.n_different, 1);
            let d = b.first_difference.unwrap();
            assert_eq!((d.word, d.xss_index), (11, at));
            assert_eq!(d.first, Some(t.xss[at - 1]));
        } else {
            assert!(b.verdict.agrees(), "{} should be untouched: {:?}", b.name, b.verdict);
        }
    }
    assert_eq!(c.whole_xss.n_different, 1);
    assert_eq!(c.whole_xss.first_difference.unwrap().xss_index, at);
    assert!(c.nxs.differences.is_empty() && c.jxs.differences.is_empty());
}

#[test]
fn a_removed_block_is_reported_missing() {
    let Some(t) = njoy_u234() else { return };
    assert!(t.jxs[jxs::NU] > 0, "U-234 is fissile: it has a NU block");
    let mut u = t.clone();
    u.jxs[jxs::NU] = 0;
    let c = compare(&t, &u);
    let nu = c.blocks.iter().find(|b| b.name == "NU").unwrap();
    assert_eq!(nu.verdict, Verdict::MissingInSecond);
    assert_eq!(c.jxs.differences, vec![(2, t.jxs[jxs::NU], 0)]);
    assert!(!c.identical());
}

fn al27_thermal_type1() -> RawAceTable {
    let tape = Tape::read(std::fs::File::open(reference_endf_dir().join("tsl-013_Al_027-ENDF8.0.endf")).unwrap()).unwrap();
    let mf7 = parse_mf7(&tape, 53).unwrap();
    let grid: Vec<f64> = (0..20).map(|i| 1.0e-4 * (4.0e4_f64).powf(i as f64 / 19.0)).collect();
    let temp = mf7.incoherent_inelastic.as_ref().unwrap().temperature_k;
    let opts = ThermalAceOptions { n_outgoing: 8, n_cosines: 4, natom: 1.0, emax_ev: 4.0, ..Default::default() };
    let ace = AceTable::thermal_from_mf7(&mf7, temp, "al27", 0, &grid, opts).unwrap();
    let text = ace.to_raw(AceFileType::Type1Ascii).unwrap().to_type1_string();
    read::parse_type1(&text).unwrap()
}

#[test]
fn thermal_tables_are_compared_by_their_own_blocks() {
    let t = al27_thermal_type1();
    let c = compare(&t, &t);
    assert!(c.identical(), "{:#?}", c.report_lines());
    assert_eq!(c.blocks.len(), THERMAL_BLOCKS.len());
    let itxe = c.blocks.iter().find(|b| b.name == "ITXE").unwrap();
    assert_eq!(itxe.verdict, Verdict::Same);
    assert!(itxe.words.0 > 0);

    let mut u = t.clone();
    let at = t.jxs[tjxs::ITXE] as usize + 3;
    u.xss[at - 1] += 1.0e-3;
    let c = compare(&t, &u);
    let itxe = c.blocks.iter().find(|b| b.name == "ITXE").unwrap();
    assert_eq!((itxe.verdict, itxe.n_different), (Verdict::Different, 1));
    assert_eq!(itxe.first_difference.unwrap().xss_index, at);
    assert!(c.blocks.iter().filter(|b| b.name != "ITXE").all(|b| b.verdict.agrees()));
}

#[test]
fn tables_of_different_classes_are_compared_as_a_whole_only() {
    let Some(ce) = njoy_u234() else { return };
    let th = al27_thermal_type1();
    let c = compare(&ce, &th);
    assert!(c.blocks.is_empty());
    assert_eq!(c.whole_xss.verdict, Verdict::Different);
    assert!(!c.identical());
}

#[test]
fn u234_built_here_is_njoy2016s_table_in_every_word() {
    let Some(njoy) = njoy_u234() else { return };
    let tape = Tape::read_file(&reference_endf("n-092_U_234-ENDF8.0.endf").expect("U-234 tape")).expect("parse U-234");
    let mat = 9225;
    let r0 = reconr(&tape, &ReconrConfig { mat, tolerance: 1.0e-3, temperature: 0.0 }).expect("RECONR");
    let r = broaden_result(&r0, 293.6);
    let deck = AceDeck::default().without_heatr().with_purr(20, 64, 10_000).with_temperature(293.6);
    let ace = build_deck(&tape, mat, &r, njoy.header.kt_mev, 0, &deck).expect("ACER");
    let text = ace.to_raw(AceFileType::Type1Ascii).unwrap().to_type1_string();
    let ours = read::parse_type1(&text).unwrap();
    let c = compare(&ours, &njoy);
    assert!(c.identical(), "{:#?}", c.report_lines());
    assert_eq!(ours.header.mat_id.trim(), "mat9225");
}
