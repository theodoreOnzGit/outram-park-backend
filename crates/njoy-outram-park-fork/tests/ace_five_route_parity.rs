// SPDX-License-Identifier: GPL-3.0

//! # The five-route library's light nuclides are NJOY2016's tables in every word (GitHub #872)
//!
//! ## Methodology
//!
//! - **Reference:** NJOY2016 2016.79 (commit `ac5adf5`, `build2`), run with
//!   the five-route campaign's deck
//!   (`crates/outram-mc-libs/verification_and_validation/openmc_godiva_cross_code/make_ace.sh`,
//!   called by `.../icsbep/five_route_keff/scripts/make_njoy_library.sh`):
//!   `RECONR (0.001) -> BROADR (293.6 K, 0.001) -> PURR (20 bins, 64
//!   ladders) -> ACER (iopt 1, newfor 1, iopp 1)`, **no HEATR**. The tables
//!   are committed in `reference-data/acer/five_route/`, each carrying its
//!   provenance in its own 70-character comment. B-10's is Type 2: NJOY2016's
//!   Type-1 formatter stops in `change` on its law-0 DLWH entry, and the
//!   campaign script reruns it with `itype = 2`.
//! - **Ours:** `process::cross_sections` on the same ENDF/B-VIII.0 tape at
//!   293.6 K with `without_heatr()` (PURR at the deck's 20 / 64 and NJOY's
//!   default 10 000 samples), written as Type 1 and read back.
//! - **Comparison:** `acer::compare`, every header field, NXS, JXS and every
//!   XSS word, with **no tolerance**. A Type-2 reference is printed through
//!   Type 1 first (integer words with our table's mask, reals to 12 figures),
//!   which is what NJOY2016's own Type-1 writer would have printed.
//! - **Pass:** `AceComparison::identical()`.
//!
//! Each nuclide gates the mechanisms #872 fixed:
//!
//! | nuclide | what it exercises |
//! |---|---|
//! | H-1 | the MF=6 LAW=2 capture photon; LDLW/DLW with NR = 0; the capture-recoil deuteron (`acelcp`) |
//! | F-19 | `convr`'s corrected MF=12 totals (GPD); photon yields with two interpolation regions |
//! | Al-27 | discrete levels given as MF=6 LAW=2 neutrons (AND); MF=6 LAW=2/4 charged particles |
//! | B-10 | `unionx`'s photon discontinuities in the energy grid; MF=6 LAW=0 particles |
//! | O-16 | anisotropic MF=14 photons (`ptleg`, ANDP); MF=4 LTT=1 charged particles; MF=13 windows |
//!
//! ## Results (2026-10-14)
//!
//! All five identical in every word. The full library, all 12 five-route
//! nuclides (U-234/235/238, F-19, O-16, H-1, Al-27, Si-28/29/30, Mn-55,
//! B-10) at 293.6 K and at 0 K (`RECONR -> ACER`), is identical too: 24 of 24
//! tables. Record:
//! `crates/njoy-outram-park-fork/verification_and_validation/ace_five_route_parity_2026_10_14.md`.

use njoy_outram_park_fork::acer::compare::compare;
use njoy_outram_park_fork::acer::photon_read::{decode_photon_production, AcePhotonEntry};
use njoy_outram_park_fork::acer::read::{self, AceFileType};
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::process::{self, CrossSectionSettings};
use njoy_outram_park_fork::reference_data::{reference_endf, reference_file_or_skip};
use uom::si::f64::ThermodynamicTemperature as Temperature;
use uom::si::thermodynamic_temperature::kelvin;

fn identical_to_njoy2016(name: &str, tape_file: &str) {
    let Some(reference) =
        reference_file_or_skip("acer", &format!("five_route/{name}_293.6K_njoy2016.ace.gz"), "ace-five-route-parity")
    else {
        return;
    };
    let Some(tape_path) = reference_endf(tape_file) else {
        eprintln!("SKIP: {tape_file} not in reference-data/endf");
        return;
    };
    let tape = Tape::read_file(&tape_path).expect("parse tape");
    let settings = CrossSectionSettings::default().without_heatr();
    let ace = process::ace(&tape, Temperature::new::<kelvin>(293.6), &settings).expect("ours");
    let ours = read::parse_type1(&ace.to_type1_text().expect("type 1")).expect("re-read ours");
    let mut njoy = read::read_library_from_bytes(std::fs::read(&reference).expect("read reference"), name)
        .expect("read NJOY2016's table")
        .remove(0);
    if njoy.file_type == AceFileType::Type2Binary {
        assert_eq!(njoy.xss.len(), ace.xss_is_int.len(), "{name}: XSS lengths differ");
        njoy.xss_is_int = Some(ace.xss_is_int.clone());
        njoy = read::parse_type1(&njoy.to_type1_string()).expect("NJOY2016's Type 2 as Type 1");
    }
    let c = compare(&ours, &njoy);
    for l in c.report_lines() {
        println!("  {name}: {l}");
    }
    assert!(c.identical(), "{name}: {:#?}", c.report_lines());
    // The photon production this crate now writes must also read back with
    // its own reader, the one the transport route uses, and decode as
    // NJOY2016's does: the same entries, the same anisotropic ones.
    let mine = decode_photon_production(&ours).expect("our photon blocks decode");
    let theirs = decode_photon_production(&njoy).expect("NJOY2016's photon blocks decode");
    assert_eq!(mine.len(), theirs.len(), "{name}: photon entries");
    let aniso = |v: &[AcePhotonEntry]| v.iter().filter(|e| e.angular.is_some()).count();
    assert_eq!(aniso(&mine), aniso(&theirs), "{name}: anisotropic photons");
    println!("  {name}: {} photon entries decode, {} anisotropic", mine.len(), aniso(&mine));
}

#[test]
fn h1_is_njoy2016s_table_in_every_word() {
    identical_to_njoy2016("H1", "n-001_H_001-ENDF8.0-Beta6.endf");
}

#[test]
fn f19_is_njoy2016s_table_in_every_word() {
    identical_to_njoy2016("F19", "n-009_F_019-ENDF8.0.endf");
}

#[test]
fn al27_is_njoy2016s_table_in_every_word() {
    identical_to_njoy2016("Al27", "n-013_Al_027-ENDF8.0.endf");
}

#[test]
fn b10_is_njoy2016s_table_in_every_word() {
    identical_to_njoy2016("B10", "n-005_B_010-ENDF8.0.endf");
}

#[test]
fn o16_is_njoy2016s_table_in_every_word() {
    identical_to_njoy2016("O16", "n-008_O_016-ENDF8.0.endf");
}
