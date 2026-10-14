//! **The large thermal tables, word for word against NJOY2016** (GitHub #865,
//! #866): the continuous secondary-energy form (`iwt = 2`, IFENG = 2, OpenMC's
//! default) and OpenMC's 64 outgoing energies, whose NJOY2016 tables (0.9-6.6
//! MB gzipped) live in the `reference-data/ace` submodule rather than in
//! `reference-data/acer/`.
//!
//! # Methodology
//!
//! As `acer_thermal_word_parity.rs`: NJOY2016 2016.79 (`ac5adf5`) on the deck
//! committed beside each table in
//! `reference-data/ace/reference-njoy/endf-b-viii.0/thermal/` (`reconr` →
//! `broadr` → `thermr` tol 0.001, `emax = 4 eV` → `acer`), against
//! `AceTable::thermal_from_mf7_on_pendf` with the same settings on the
//! incident nuclide's PENDF from this crate's RECONR + BROADR. Compared by
//! `acer::compare` (header incl. IZ/AW, NXS, JXS, every block, whole XSS) and
//! as text byte for byte except line 1's date and line 2's comment.
//!
//! | table | T (K) | bins x cosines | natom | `iwt` |
//! |---|---|---|---|---|
//! | Al-27 (incident Al-27) | 20 | 16 x 16 | 1 | 2 |
//! | H in H₂O (incident H-1) | 283.6 | 64 x 16 | 2 | 0 |
//! | H in H₂O (incident H-1) | 283.6 | 64 x 16 | 2 | 2 |
//!
//! Pass criterion: identical, no tolerance. The tests skip only when the
//! submodule is absent (CI fetches it).
//!
//! # Results (2026-10-14)
//!
//! All three identical in every compared word and every byte outside the date
//! and comment (measured on the same tables before they were stamped and
//! moved into the submodule; the stamped decks differ only in the comment
//! card, and the data lines were checked identical).

use njoy_outram_park_fork::acer::compare::compare;
use njoy_outram_park_fork::acer::read::{self, AceFileType};
use njoy_outram_park_fork::acer::thermal::{ace_incident_grid, IncidentPendf, InelasticForm, ThermalAceOptions};
use njoy_outram_park_fork::acer::AceTable;
use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{ace_reference_file_or_skip, reference_endf};
use njoy_outram_park_fork::thermr::mf7::parse_mf7;

#[allow(clippy::too_many_arguments)]
fn check(table: &str, incident: &str, mat: i32, tsl: &str, tmat: i32, temp_k: f64, nbins: usize, natom: f64, form: InelasticForm, name: &str, za: i32) {
    let Some(path) = ace_reference_file_or_skip(&format!("reference-njoy/endf-b-viii.0/thermal/{table}"), "thermal parity (large)") else { return };
    let njoy_bytes = read::inflate_gzip(&std::fs::read(&path).unwrap(), &path).unwrap();
    let njoy = read::read_library_from_bytes(njoy_bytes.clone(), table).unwrap().pop().unwrap();
    let it = Tape::read_file(&reference_endf(incident).expect("incident tape")).unwrap();
    let r = broaden_result(&reconr(&it, &ReconrConfig { mat, tolerance: 1.0e-3, temperature: 0.0 }).unwrap(), temp_k);
    let elastic = r.sections.iter().find(|s| s.mt.number() == 2).expect("MT=2").pairs.clone();
    let inc = IncidentPendf { mat, awr: r.material.awr, moderator_za: vec![za], elastic };
    let mf7 = parse_mf7(&Tape::read_file(&reference_endf(tsl).expect("thermal tape")).unwrap(), tmat).unwrap();
    let opts = ThermalAceOptions { n_outgoing: nbins, n_cosines: 16, emax_ev: 4.0, natom, form };
    let ace = AceTable::thermal_from_mf7_on_pendf(&mf7, temp_k, name, 0, &ace_incident_grid(4.0), opts, 0.001, &inc).unwrap();
    let ours_text = ace.to_raw(AceFileType::Type1Ascii).unwrap().to_type1_string();
    let ours = read::parse_type1(&ours_text).unwrap();
    let cmp = compare(&ours, &njoy);
    let lines = cmp.report_lines();
    println!("{table}:");
    for l in &lines {
        println!("  {l}");
    }
    assert!(cmp.identical(), "{table} differs from NJOY2016: {lines:#?}");
    let njoy_text = String::from_utf8(njoy_bytes).unwrap();
    let (ol, nl): (Vec<&str>, Vec<&str>) = (ours_text.lines().collect(), njoy_text.lines().collect());
    assert_eq!(ol.len(), nl.len(), "{table}: line count");
    assert_eq!(ol[0][..35], nl[0][..35], "{table}: ZAID, AWR, kT as text");
    assert_eq!(ol[1][70..], nl[1][70..], "{table}: material tag as text");
    if let Some(k) = (2..ol.len()).find(|&k| ol[k] != nl[k]) {
        panic!("{table}: line {} differs:\n ours {:?}\n njoy {:?}", k + 1, ol[k], nl[k]);
    }
}

#[test]
fn al27_continuous_form_iwt2_is_njoy2016s_table() {
    check("Al27_tsl_20K_16b_iwt2.ace.gz", "n-013_Al_027-ENDF8.0.endf", 1325, "tsl-013_Al_027-ENDF8.0.endf", 53, 20.0, 16, 1.0, InelasticForm::Continuous, "al27", 13027);
}

#[test]
fn h_in_h2o_64_skewed_bins_iwt0_is_njoy2016s_table() {
    check("HinH2O_283.6K_64b_iwt0.ace.gz", "n-001_H_001-ENDF8.0-Beta6.endf", 125, "tsl-HinH2O.endf", 1, 283.6, 64, 2.0, InelasticForm::Skewed, "hh2o", 1001);
}

#[test]
fn h_in_h2o_64_continuous_iwt2_is_njoy2016s_table() {
    check("HinH2O_283.6K_64b_iwt2.ace.gz", "n-001_H_001-ENDF8.0-Beta6.endf", 125, "tsl-HinH2O.endf", 1, 283.6, 64, 2.0, InelasticForm::Continuous, "hh2o", 1001);
}
