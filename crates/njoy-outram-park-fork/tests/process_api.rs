//! **The four high-level entry points against NJOY2016** (GitHub #871):
//! `process::cross_sections` (a), `process::ace` (b),
//! `process::thermal_scattering` (c) and `process::thermal_ace` (d).
//!
//! # Methodology
//!
//! - **(b) against NJOY2016's U-234 table.** ENDF/B-VIII.0 U-234 (MAT 9225)
//!   through `process::ace` at 293.6 K with the default settings except
//!   HEATR, which NJOY2016's reference deck did not run
//!   (`RECONR+BROADR+PURR+ACER`, PURR 20 bands / 64 ladders); compared with
//!   `reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/U234.ace.gz` by
//!   `acer::compare` (header, NXS, JXS, every block, the whole XSS). Pass:
//!   identical in every compared word. The default physics is checked on
//!   the same run: PURR's tables are present.
//! - **(d) against NJOY2016 on the same LEAPR deck.** NJOY2016 2016.79
//!   (`ac5adf5`) ran `leapr → reconr → broadr → thermr → acer` on the
//!   ENDF/B-VIII.0 H-in-H₂O deck cut to 293.6 K
//!   (`reference-data/leapr/tsl-HinH2O-293.6K-njoy2016-leapr.njoy-input`)
//!   and H-1 (`n-001_H_001-ENDF8.0-Beta6.endf`), with THERMR's `natom = 2`
//!   and `emax = 10.00008` eV (the law's `B(6)` and `B(4)`), tol 0.001, 16
//!   cosines, ACER 16 outgoing energies, `iwt = 1`. Deck:
//!   `reference-data/acer/thermal_decks/hh2o_leapr_293p6k_tol0p001.njoy`;
//!   table: `reference-data/acer/hh2o_leapr_293p6k_thermal_tol0p001_njoy2016.ace.gz`.
//!   This crate runs `process::thermal_ace` on the same deck text and the
//!   same H-1 tape with the **default** settings, except the LEAPR constants
//!   (CODATA 2018, as NJOY2016 uses; the deck's `EVAL-JUN17` date would
//!   otherwise select the legacy `k_B` that reproduces the published tape)
//!   and the table name. So `natom` and `emax` come from the law, as the
//!   defaults take them. Pass: identical in every compared word, and every
//!   byte of the text but the date and the comment.
//! - **(c) from the full deck.** The committed 18-temperature deck
//!   (`src/leapr/decks/tsl-HinH2O.leapr`) at 293.6 K selects its 293.6 K
//!   block; its table must equal the cut deck's. A temperature between two of
//!   its blocks (330 K), where the deck has a spectrum per temperature, is
//!   refused.
//! - **Refusals:** a negative temperature, a tape with no material, a bad
//!   table name.
//!
//! # Results (2026-10-14)
//!
//! - (b): U-234 identical to NJOY2016's in every compared word (449 695 XSS
//!   words).
//! - (d): H in H₂O identical to NJOY2016's in every compared word and every
//!   byte outside the date and comment; `natom = 2`, `emax = 10.00008` eV
//!   from `B(6)`, `B(4)`.
//! - (c): the full deck's 293.6 K table equals the cut deck's word for word;
//!   330 K is refused.
//! - All refusals return `NjoyError`, none panics.

use njoy_outram_park_fork::acer::compare::compare;
use njoy_outram_park_fork::acer::read::{self, RawAceTable};
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::vintage::PhysicalConstants;
use njoy_outram_park_fork::process::{self, CrossSectionSettings, ThermalSettings};
use njoy_outram_park_fork::reference_data::{ace_reference_file_or_skip, reference_data_dir, reference_endf, reference_file_or_skip};
use njoy_outram_park_fork::units::Temperature;
use njoy_outram_park_fork::NjoyError;
use uom::si::thermodynamic_temperature::kelvin;

fn kelvins(t: f64) -> Temperature {
    Temperature::new::<kelvin>(t)
}

fn tape(name: &str) -> Tape {
    Tape::read_file(&reference_endf(name).expect("reference tape")).expect("parse tape")
}

/// The LEAPR deck NJOY read: from the `leapr` line to the `stop` line.
fn leapr_section(text: &str) -> String {
    let start = text.find("leapr\n").expect("a leapr line");
    let body = &text[start..];
    let end = body.find("\nstop").expect("a stop line");
    format!("{}\nstop\n", &body[..end])
}

fn h2o_cut_deck() -> Option<String> {
    let p = reference_file_or_skip("leapr", "tsl-HinH2O-293.6K-njoy2016-leapr.njoy-input", "process-api")?;
    Some(leapr_section(&std::fs::read_to_string(p).expect("deck")))
}

fn njoy_settings() -> ThermalSettings {
    ThermalSettings::default().with_leapr_constants(PhysicalConstants::Codata2018).with_table_name("hh2o")
}

#[test]
fn ace_reproduces_njoy2016s_u234_table_in_every_word() {
    let Some(p) = ace_reference_file_or_skip("reference-njoy/endf-b-viii.0/293.6K/U234.ace.gz", "process-api") else {
        return;
    };
    let njoy: RawAceTable =
        read::read_library_from_bytes(std::fs::read(&p).unwrap(), "U234.ace.gz").unwrap().pop().unwrap();
    let u234 = tape("n-092_U_234-ENDF8.0.endf");
    let settings = CrossSectionSettings::default().without_heatr();
    let xs = process::cross_sections(&u234, kelvins(293.6), &settings).expect("(a)");
    assert_eq!(xs.mat, 9225);
    assert!(xs.urr.is_some(), "PURR is on by default and U-234 has an unresolved range");
    assert!(xs.heating.is_none(), "HEATR was ablated");
    let ace = xs.to_ace().expect("(b)");
    let ours = read::parse_type1(&ace.to_type1_text().unwrap()).unwrap();
    let c = compare(&ours, &njoy);
    for l in c.report_lines() {
        println!("  {l}");
    }
    assert!(c.identical(), "{:#?}", c.report_lines());
    // `process::ace` is the same two calls.
    let again = process::ace(&u234, kelvins(293.6), &settings).expect("(b) in one call");
    assert_eq!(again.to_type1_text().unwrap(), ace.to_type1_text().unwrap());
    // The Type-2 bytes read back to the same table. Type 2 carries the
    // unrounded doubles (`sigfig`'s 1e-13 bias, where Type 1 prints 12
    // figures), so real words agree to Type 1's 12 figures, not bit for bit.
    // Integer words agree through `nint`, which is how Type 1 writes them:
    // MTRP keeps `acelpp`'s flag digit as a fraction (`acefc.f90:8490`;
    // U-234's fourth MT=3 photon, an MF=15 continuum, is 3004.1 in Type 2
    // and `3004` in Type 1), as NJOY2016's own Type 2 does (2026-10-15).
    let t2 = read::read_library_from_bytes(ace.to_type2_bytes().unwrap(), "u234.type2").unwrap().pop().unwrap();
    assert_eq!((t2.nxs, t2.jxs, t2.xss.len()), (ours.nxs, ours.jxs, ours.xss.len()));
    let mut worst = 0.0f64;
    for ((&a, &b), &is_int) in t2.xss.iter().zip(&ours.xss).zip(&ace.xss_is_int) {
        if is_int {
            assert_eq!(a.round(), b, "an integer word reads back as its nint");
        } else {
            worst = worst.max((a - b).abs() / b.abs().max(1e-300));
        }
    }
    assert!(worst < 1e-11, "Type 2 against Type 1, real words: {worst:e}");
    assert!(
        t2.xss.iter().zip(&ace.xss_is_int).any(|(&v, &i)| i && v.fract() != 0.0),
        "U-234 has an MF=15 photon, so one MTRP word is fractional, as NJOY2016's"
    );
}

#[test]
fn the_default_settings_run_heatr_and_purr() {
    let s = CrossSectionSettings::default();
    assert!(s.heatr && s.purr.is_some(), "physics on by default: {s:?}");
    let h2 = tape("n-001_H_002-ENDF8.0.endf");
    let xs = process::cross_sections(&h2, kelvins(293.6), &s).expect("H-2");
    assert!(xs.heating.is_some(), "HEATR ran");
    assert!(xs.urr.is_none(), "H-2 has no unresolved range");
    assert!(xs.reaction(njoy_outram_park_fork::MtReaction::Mt2Elastic).is_some());
    assert!((xs.awr() - 1.996800).abs() < 1e-5 && xs.za() == 1002.0, "{} {}", xs.awr(), xs.za());
    let ace = xs.to_ace().unwrap();
    assert!(ace.zaid.trim().starts_with("1002.00c"), "{}", ace.zaid);
    // Cold: no BROADR, no PURR.
    let cold = process::cross_sections(&h2, kelvins(0.0), &s).expect("H-2 at 0 K");
    assert!(cold.urr.is_none() && cold.heating.is_some());
}

#[test]
fn bad_inputs_are_refused_not_panicked_on() {
    let h2 = tape("n-001_H_002-ENDF8.0.endf");
    let e = process::cross_sections(&h2, kelvins(-1.0), &CrossSectionSettings::default()).unwrap_err();
    assert!(matches!(e, NjoyError::InvalidInput(_)), "{e}");
    let empty = Tape::read(std::io::Cursor::new(b"")).unwrap();
    let e = process::ace(&empty, kelvins(293.6), &CrossSectionSettings::default()).unwrap_err();
    assert!(matches!(e, NjoyError::InvalidInput(_)), "{e}");
    let e = process::thermal_scattering(&h2, "leapr\n", kelvins(0.0), &ThermalSettings::default()).unwrap_err();
    assert!(matches!(e, NjoyError::InvalidInput(_)), "{e}");
}

#[test]
fn thermal_ace_on_a_leapr_deck_is_njoy2016s_table_in_every_word() {
    let Some(deck) = h2o_cut_deck() else { return };
    let path = reference_data_dir("acer").join("hh2o_leapr_293p6k_thermal_tol0p001_njoy2016.ace.gz");
    let njoy_text = String::from_utf8(read::inflate_gzip(&std::fs::read(&path).unwrap(), &path).unwrap()).unwrap();
    let njoy = read::parse_type1(&njoy_text).unwrap();
    let h1 = tape("n-001_H_001-ENDF8.0-Beta6.endf");
    let law = process::thermal_scattering(&h1, &deck, kelvins(293.6), &njoy_settings()).expect("(c)");
    assert_eq!((law.mat, law.thermr.natom, law.thermr.emax_ev), (1, 2.0, 10.00008), "natom, emax from B(6), B(4)");
    assert_eq!(law.thermr.incident.as_ref().map(|i| i.mat), Some(125), "THERMR ran on H-1's PENDF");
    let ace = law.to_ace().expect("(d)");
    let ours_text = ace.to_type1_text().unwrap();
    let (ol, nl): (Vec<&str>, Vec<&str>) = (ours_text.lines().collect(), njoy_text.lines().collect());
    assert_eq!(ol.len(), nl.len(), "line count");
    assert_eq!(ol[0][..35], nl[0][..35], "ZAID, AWR and kT as text");
    assert_eq!(ol[1][70..], nl[1][70..], "the material tag");
    if let Some(k) = (2..ol.len()).find(|&k| ol[k] != nl[k]) {
        panic!("line {} differs:\n ours {:?}\n njoy {:?}", k + 1, ol[k], nl[k]);
    }
    let c = compare(&read::parse_type1(&ours_text).unwrap(), &njoy);
    for l in c.report_lines() {
        println!("  {l}");
    }
    assert!(c.identical(), "{:#?}", c.report_lines());
    // (d) in one call is the same table.
    let again = process::thermal_ace(&h1, &deck, kelvins(293.6), &njoy_settings()).expect("(d) in one call");
    assert_eq!(again.to_type1_text().unwrap(), ours_text);
}

#[test]
fn the_full_deck_picks_its_block_and_refuses_between_spectra() {
    let Some(cut) = h2o_cut_deck() else { return };
    let full = include_str!("../src/leapr/decks/tsl-HinH2O.leapr");
    let h1 = tape("n-001_H_001-ENDF8.0-Beta6.endf");
    let s = njoy_settings().with_emax(njoy_outram_park_fork::units::NeutronEnergy::new::<uom::si::energy::electronvolt>(1.0));
    let a = process::thermal_ace(&h1, full, kelvins(293.6), &s).expect("full deck at 293.6 K");
    let b = process::thermal_ace(&h1, &cut, kelvins(293.6), &s).expect("cut deck at 293.6 K");
    let c = compare(&read::parse_type1(&a.to_type1_text().unwrap()).unwrap(), &read::parse_type1(&b.to_type1_text().unwrap()).unwrap());
    assert!(c.identical(), "the full deck's 293.6 K block is the cut deck: {:#?}", c.report_lines());
    let e = process::thermal_scattering(&h1, full, kelvins(330.0), &s).unwrap_err();
    assert!(matches!(e, NjoyError::NotPorted(_)), "330 K lies between two spectra: {e}");
    let e = s.clone().with_table_name("x");
    let law = process::thermal_scattering(&h1, &cut, kelvins(293.6), &e).unwrap();
    assert!(matches!(law.to_ace().unwrap_err(), NjoyError::InvalidInput(_)));
}
