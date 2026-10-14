//! **Thermal S(α,β) ACE tables built here are NJOY2016's, word for word**
//! (GitHub #865).
//!
//! # Methodology
//!
//! Each case runs NJOY2016 2016.79 (`ac5adf5`) on the deck committed beside
//! its table in `reference-data/acer/thermal_decks/`:
//! `reconr` (0.001) → `broadr` (the S(α,β) base temperature) → `thermr`
//! (16 cosines, `iform = 0`, card-4 `tol`, `emax = 4 eV`) → `acer` (`iopt = 2`,
//! 16 equiprobable outgoing energies, `iwt = 1`), every unit positive (text),
//! as OpenMC's `make_ace_thermal` writes it. This crate builds the same table:
//! RECONR + BROADR of the same incident tape, `parse_mf7` of the same thermal
//! tape, `AceTable::thermal_from_mf7_on_pendf` with the same `tol`, `natom`
//! and `emax`, on the incident energies `ace_incident_grid` keeps. The two are
//! compared by `acer::compare`: header (ZAID, AWR, kT, material, IZ/AW), NXS,
//! JXS entry by entry, every thermal block and the whole XSS word by word;
//! and as text, byte for byte, but for the date and the comment.
//!
//! | case | incident | T (K) | tol | natom | coherent elastic |
//! |---|---|---|---|---|---|
//! | Al-27 (MAT 53) | Al-27 (1325) | 20 | 0.05 | 1 | yes |
//! | Al-27 | Al-27 | 20 | 0.001 | 1 | yes |
//! | H in H₂O (MAT 1) | H-1 (125) | 283.6 | 0.001 | 2 | no |
//! | crystalline graphite (MAT 30) | C-12 (625) | 296 | 0.001 | 1 | yes |
//! | Al-27, `iwt = 0` (skewed bins, IFENG = 1) | Al-27 | 20 | 0.001 | 1 | yes |
//! | H in ZrH (MAT 7), incoherent elastic (`IDPNC = 3`) | H-1 (125) | 296 | 0.001 | 1 | no |
//!
//! All `iwt = 1` (IFENG = 0) except the skewed Al-27 case. `iwt = 2` (IFENG = 2, OpenMC's
//! default) was measured identical too, on Al-27 at 16 bins and on H in H₂O
//! at 64, but its tables (2-7 MB gzipped) are not committed here; see the
//! V&V record.
//!
//! Pass criterion: identical in every compared word and every byte outside
//! the date and comment fields. No tolerance.
//!
//! # Results (2026-10-14)
//!
//! ~~All five identical in every compared word, in 13 s.~~ **UPDATED 2026-10-14:**
//! six cases since H in ZrH was added (#866), all identical in every compared
//! word and every byte but date and comment (40 s on 2 threads). Before this date the
//! port differed from NJOY2016 in every block but ITIE.
//!
//! H in ZrH (added later 2026-10-14, GitHub #866; ACER card 9 `mte = 223`,
//! `ielas = 1`) is identical too, ITCE/ITCX/ITCA included (31 s). Before
//! THERMR's `iel` MF=3 and ACER's read-back were ported, ITCX differed in all
//! 106 words (worst 2.24e-3) and ITCA in all 1 696 (worst 5.0e-8). Record:
//! `verification_and_validation/acer_thermal_incoherent_elastic_2026_10_14.md`.
//! Mixed elastic (`IDPNC = 5`) has no case: no `LTHR = 3` tape is held.
//!
//! The record of what was found on the way, and how (an instrumented NJOY2016
//! build logging `sig`, `sigl` and `tpend`), is
//! `verification_and_validation/acer_thermal_word_parity_2026_10_14.md`.

use njoy_outram_park_fork::acer::compare::compare;
use njoy_outram_park_fork::acer::read::{self, AceFileType, RawAceTable};
use njoy_outram_park_fork::acer::thermal::{ace_incident_grid, IncidentPendf, InelasticForm, ThermalAceOptions};
use njoy_outram_park_fork::acer::AceTable;
use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{reference_data_dir, reference_endf};
use njoy_outram_park_fork::thermr::mf7::parse_mf7;

struct Case {
    njoy: &'static str,
    incident: &'static str,
    mat: i32,
    tsl: &'static str,
    tmat: i32,
    temp_k: f64,
    tol: f64,
    natom: f64,
    name: &'static str,
    form: InelasticForm,
    /// ACER card 8a in the deck.
    za: i32,
}

fn check(c: Case) {
    let bytes = std::fs::read(reference_data_dir("acer").join(c.njoy)).expect("reference table");
    let njoy: RawAceTable = read::read_library_from_bytes(bytes, c.njoy).expect("parse").pop().unwrap();
    let inc_tape = Tape::read_file(&reference_endf(c.incident).expect("incident tape")).unwrap();
    let r0 = reconr(&inc_tape, &ReconrConfig { mat: c.mat, tolerance: 1.0e-3, temperature: 0.0 }).unwrap();
    let r = broaden_result(&r0, c.temp_k);
    let elastic = r.sections.iter().find(|s| s.mt.number() == 2).expect("MT=2").pairs.clone();
    let incident = IncidentPendf { mat: c.mat, awr: r.material.awr, moderator_za: vec![c.za], elastic };
    let tsl = Tape::read_file(&reference_endf(c.tsl).expect("thermal tape")).unwrap();
    let mf7 = parse_mf7(&tsl, c.tmat).unwrap();
    // The grid ACER keeps, from this crate's rule (not copied from NJOY's
    // table, so the rule itself is checked by ITIE).
    let grid = ace_incident_grid(4.0);
    let opts = ThermalAceOptions { n_outgoing: 16, n_cosines: 16, emax_ev: 4.0, natom: c.natom, form: c.form };
    let ace = AceTable::thermal_from_mf7_on_pendf(&mf7, c.temp_k, c.name, 0, &grid, opts, c.tol, &incident).unwrap();
    let ours = read::parse_type1(&ace.to_raw(AceFileType::Type1Ascii).unwrap().to_type1_string()).unwrap();
    // Byte for byte as text, the date (line 1 after column 35) and the
    // comment (line 2, columns 1-70) excepted: those are each code's own
    // provenance. Everything else, IZ/AW included, must be NJOY's bytes.
    let ours_text = ace.to_raw(AceFileType::Type1Ascii).unwrap().to_type1_string();
    let path = reference_data_dir("acer").join(c.njoy);
    let njoy_text = String::from_utf8(read::inflate_gzip(&std::fs::read(&path).unwrap(), &path).unwrap()).unwrap();
    let (ol, nl): (Vec<&str>, Vec<&str>) = (ours_text.lines().collect(), njoy_text.lines().collect());
    assert_eq!(ol.len(), nl.len(), "{}: line count", c.njoy);
    assert_eq!(ol[0][..35], nl[0][..35], "{}: ZAID, AWR and kT as text", c.njoy);
    assert_eq!(ol[1][70..], nl[1][70..], "{}: the material tag as text", c.njoy);
    if let Some(k) = (2..ol.len()).find(|&k| ol[k] != nl[k]) {
        panic!("{}: line {} differs:\n ours {:?}\n njoy {:?}", c.njoy, k + 1, ol[k], nl[k]);
    }
    let cmp = compare(&ours, &njoy);
    let lines = cmp.report_lines();
    println!("{}:", c.njoy);
    for l in &lines {
        println!("  {l}");
    }
    assert!(cmp.identical(), "{} differs from NJOY2016: {lines:#?}", c.njoy);
}

#[test]
fn al27_at_20k_tol_0p05_is_njoy2016s_table() {
    check(Case {
        njoy: "al27_20k_thermal_tol0p05_njoy2016.ace.gz",
        incident: "n-013_Al_027-ENDF8.0.endf",
        mat: 1325,
        tsl: "tsl-013_Al_027-ENDF8.0.endf",
        tmat: 53,
        temp_k: 20.0,
        tol: 0.05,
        natom: 1.0,
        name: "al27",
        za: 13027,
        form: InelasticForm::Equiprobable,
    });
}

#[test]
fn al27_at_20k_tol_0p001_is_njoy2016s_table() {
    check(Case {
        njoy: "al27_20k_thermal_tol0p001_njoy2016.ace.gz",
        incident: "n-013_Al_027-ENDF8.0.endf",
        mat: 1325,
        tsl: "tsl-013_Al_027-ENDF8.0.endf",
        tmat: 53,
        temp_k: 20.0,
        tol: 0.001,
        natom: 1.0,
        name: "al27",
        za: 13027,
        form: InelasticForm::Equiprobable,
    });
}

#[test]
fn h_in_h2o_at_283p6k_is_njoy2016s_table() {
    check(Case {
        njoy: "hh2o_283p6k_thermal_tol0p001_njoy2016.ace.gz",
        incident: "n-001_H_001-ENDF8.0-Beta6.endf",
        mat: 125,
        tsl: "tsl-HinH2O.endf",
        tmat: 1,
        temp_k: 283.6,
        tol: 0.001,
        natom: 2.0,
        name: "hh2o",
        za: 1001,
        form: InelasticForm::Equiprobable,
    });
}

#[test]
fn crystalline_graphite_at_296k_is_njoy2016s_table() {
    check(Case {
        njoy: "graphite_296k_thermal_tol0p001_njoy2016.ace.gz",
        incident: "n-006_C_012-ENDF8.0.endf",
        mat: 625,
        tsl: "tsl-crystalline-graphite.endf",
        tmat: 30,
        temp_k: 296.0,
        tol: 0.001,
        natom: 1.0,
        name: "grph",
        za: 6012,
        form: InelasticForm::Equiprobable,
    });
}

#[test]
fn al27_skewed_bins_iwt0_are_njoy2016s_table() {
    check(Case {
        njoy: "al27_20k_thermal_tol0p001_iwt0_njoy2016.ace.gz",
        incident: "n-013_Al_027-ENDF8.0.endf",
        mat: 1325,
        tsl: "tsl-013_Al_027-ENDF8.0.endf",
        tmat: 53,
        temp_k: 20.0,
        tol: 0.001,
        natom: 1.0,
        name: "al27",
        za: 13027,
        form: InelasticForm::Skewed,
    });
}

#[test]
fn h_in_zrh_incoherent_elastic_at_296k_is_njoy2016s_table() {
    check(Case {
        njoy: "hzrh_296k_thermal_tol0p001_njoy2016.ace.gz",
        incident: "n-001_H_001-ENDF8.0-Beta6.endf",
        mat: 125,
        tsl: "tsl-HinZrH-ENDF8.0.endf",
        tmat: 7,
        temp_k: 296.0,
        tol: 0.001,
        natom: 1.0,
        name: "hzrh",
        za: 1001,
        form: InelasticForm::Equiprobable,
    });
}
