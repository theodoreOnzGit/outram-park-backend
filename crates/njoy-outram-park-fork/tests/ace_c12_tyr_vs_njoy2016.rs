// Reads reference files or the deck work directory: needs `std` (gh:#1592).
#![cfg(feature = "std")]

// SPDX-License-Identifier: AGPL-3.0-only
//! **C-12 ACE: the TYR block against NJOY2016.80** (gh:#338).
//!
//! # What was reported
//!
//! The C-12 table this crate wrote on 2026-09-26 (header stamp `njoy-outram-park-fork
//! v0.0.3 6b71d21+`, through `NuclearDataLibrary`, which then assembled its own table with
//! `AceTable::from_reconr_full`) recorded **TYR = 0** for MT=28 (n,np) and MT=91 (continuum
//! inelastic): a transport code reading it would treat both as absorption.
//!
//! # Upstream
//!
//! `acefc.f90`'s `acelod` sets TYR from the MT (`:5859-5870`, the `mth.le.91` neutron
//! yield, `xss(tyr+ir-1)=n`) and signs it by the frame of the MF=4/MF=6 data; `acelf6`
//! (`:7273-7279`) takes it from the MF=6 neutron yield. C-12 ENDF/B-VIII.0 (MAT 625) gives
//! MT=28 an MF=6 neutron product and MT=91 MF=4 + MF=5 (LF=9), so both emit one neutron.
//!
//! # Methodology
//!
//! NJOY2016.80 (`ab27c64`, `build2/njoy`) was run on `reference-data/endf/n-006_C_012-ENDF8.0.endf`
//! with `reconr 20 21 / 625 0 / 0.001 / 0`, `broadr 20 21 22 / 625 1 / 0.001 / 293.6 / 0`,
//! `acer 20 22 0 23 24 / 1 1 1 .00 / 625 293.6`, on 2026-11-24. Its listing's reaction table
//! (MT, TYR) is [`NJOY_MTR_TYR`] below. This test builds the table through the two routes this
//! crate offers, `NuclearDataLibrary::to_ace` (`process::ace`: RECONR, BROADR, PURR, HEATR,
//! ACER) and the deck driver on NJOY's own deck, and compares MTR and TYR word for word.
//!
//! # Prediction (2026-11-24, before the run)
//!
//! NJOY writes TYR = 1 for both MTs (from `acelod`). The deck route is NJOY's table in every
//! word; `process::ace` replaced `from_reconr_full` on 2026-10-17 (gh:#933), and continuum
//! MF=4+5 reactions got `acelod`'s TYR on 2026-10-14 (Mg-24), so it should agree too.
//!
//! # Result (2026-11-24)
//!
//! NJOY2016.80 writes **TYR = 1** for MT=28 and MT=91. Both routes of this crate reproduce all
//! 19 (MT, TYR) pairs; the deck route's `tape23` is NJOY's byte for byte (`cmp`, 4 181 328
//! bytes). So the 2026-09-26 TYR = 0 was a defect of the port as it stood then, not a property
//! of the evaluation, and it is gone from both current routes. Which change removed it was not
//! re-run; the likely one is `build_emissions` taking `acelod`'s TYR for MF=4 + MF=5 continuum
//! reactions (2026-10-14, found on Mg-24, which had the same TYR = 0 on MT=16/22/28/91).

use njoy_outram_park_fork::acer::read::parse_type1;
use njoy_outram_park_fork::deck::{run_deck, DeckOptions};
use njoy_outram_park_fork::interface::NuclearDataLibrary;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

const TAPE: &str = "n-006_C_012-ENDF8.0.endf";

/// NJOY2016.80's reaction table for C-12 at 293.6 K: (MT, TYR), as its ACER listing prints it.
const NJOY_MTR_TYR: [(i32, i32); 19] = [
    (5, -101),
    (28, 1),
    (51, -1),
    (52, -1),
    (53, -1),
    (54, -1),
    (55, -1),
    (56, -1),
    (57, -1),
    (58, -1),
    (59, -1),
    (60, -1),
    (61, -1),
    (62, -1),
    (91, 1),
    (102, 0),
    (103, 0),
    (104, 0),
    (107, 0),
];

const DECK: &str = "reconr\n 20 21/\n 'c12 pendf'/\n 625 0/\n 0.001/\n 0/\nbroadr\n 20 21 22/\n 625 1/\n \
0.001/\n 293.6/\n 0/\nacer\n 20 22 0 23 24/\n 1 0 1 .00/\n 'c12 293.6K'/\n 625 293.6/\n /\n /\nstop\n";

/// The (MT, TYR) pairs of a Type-1 table: NXS(4) reactions, MTR at JXS(3), TYR at JXS(5).
fn mtr_tyr(text: &str) -> Vec<(i32, i32)> {
    let t = parse_type1(text).expect("ACE parses");
    let ntr = t.nxs[3] as usize;
    let mtr = t.slice_at(t.jxs[2], ntr).expect("MTR block");
    let tyr = t.slice_at(t.jxs[4], ntr).expect("TYR block");
    mtr.iter().zip(tyr).map(|(&m, &y)| (m as i32, y as i32)).collect()
}

#[test]
fn c12_tyr_block_is_njoy2016s_on_both_routes() {
    let Some(endf) = reference_endf_or_skip(TAPE, "C-12 TYR (gh:#338)") else { return };

    let lib = NuclearDataLibrary::from_file(&endf)
        .unwrap()
        .reconstruct(0.001)
        .unwrap()
        .broaden(ThermodynamicTemperature::new::<kelvin>(293.6))
        .unwrap();
    let api = mtr_tyr(&lib.to_ace().unwrap().to_type1_text().unwrap());
    println!("process::ace  {api:?}");
    assert_eq!(api, NJOY_MTR_TYR, "NuclearDataLibrary's C-12 MTR/TYR differ from NJOY2016's");

    let dir = std::env::temp_dir().join(format!("njoy_c12_tyr_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(&endf, dir.join("tape20")).unwrap();
    let run = run_deck(
        DECK,
        &DeckOptions { workdir: Some(dir.clone()), parse_only: false, run_date: None },
    )
    .unwrap();
    assert_eq!(run.blocked, None, "the deck must run to its end");
    let deck = mtr_tyr(&std::fs::read_to_string(dir.join("tape23")).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
    println!("deck driver   {deck:?}");
    assert_eq!(deck, NJOY_MTR_TYR, "the deck route's C-12 MTR/TYR differ from NJOY2016's");

    // The two MTs of gh:#338 emit one neutron each, lab frame (positive sign).
    for mt in [28, 91] {
        let tyr = api.iter().find(|p| p.0 == mt).map(|p| p.1);
        assert_eq!(tyr, Some(1), "MT={mt} must emit one neutron (TYR = 1)");
    }
}
