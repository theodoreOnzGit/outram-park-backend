// SPDX-License-Identifier: GPL-3.0

//! **THERMR's card-4 `tol` reaches the thermal ACE writer.**
//!
//! `AceTable::thermal_from_mf7` builds with a fixed `tol = 0.05`
//! (`acer::thermal::CALCEM_TOL`). OpenMC's `make_ace_thermal`, the driver behind
//! the NJOY2016 reference tables of the five-route ICSBEP study, passes
//! `tol = 0.001`. A Rust table built at 0.05 against an NJOY2016 table built at
//! 0.001 differs by the tolerance alone, which is how the study's H(H2O)
//! emission bins came to disagree more than the recorded parity (2026-09-29,
//! `outram-mc-libs/verification_and_validation/icsbep/five_route_keff_2026_09_29.md`).
//!
//! Two properties are pinned:
//!
//! 1. **The default path is unchanged.** `thermal_from_mf7` equals
//!    `thermal_from_mf7_with_tolerance(.., CALCEM_TOL)` word for word, so every
//!    byte-parity gate taken at 0.05 still measures what it measured.
//! 2. **The tolerance reaches `calcem`.** Building at 0.001 changes the
//!    emission block (ITXE) **and the inelastic cross section (ITIX)**, which is
//!    `calcem`'s own `xsi` since 2026-09-29, and leaves the incident-energy grid
//!    (ITIE), which the caller supplies, identical.

use std::fs::File;

use njoy_outram_park_fork::acer::thermal::{jxs, ThermalAceOptions, CALCEM_TOL};
use njoy_outram_park_fork::acer::AceTable;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::thermr::mf7::parse_mf7;

const AL_MAT: i32 = 53;

fn build(tol: Option<f64>) -> Option<AceTable> {
    let p = njoy_outram_park_fork::reference_data::reference_endf_dir()
        .join("tsl-013_Al_027-ENDF8.0.endf");
    let tape = Tape::read(File::open(p).ok()?).ok()?;
    let mf7 = parse_mf7(&tape, AL_MAT).ok()?;
    let ne = 24;
    let (elo, ehi) = (1.0e-4_f64, 4.0_f64);
    let grid: Vec<f64> = (0..ne)
        .map(|i| elo * (ehi / elo).powf(i as f64 / (ne - 1) as f64))
        .collect();
    let temp = mf7.incoherent_inelastic.as_ref()?.temperature_k;
    let opts = ThermalAceOptions {
        n_outgoing: 16,
        n_cosines: 8,
        natom: 1.0,
        emax_ev: 4.0,
        ..Default::default()
    };
    match tol {
        None => AceTable::thermal_from_mf7(&mf7, temp, "al27", 0, &grid, opts).ok(),
        Some(t) => {
            AceTable::thermal_from_mf7_with_tolerance(&mf7, temp, "al27", 0, &grid, opts, t).ok()
        }
    }
}

#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "builds three thermal ACE tables from the Al-27 TSL tape; runs by default"
)]
fn thermr_tol_reaches_calcem_and_the_default_is_unchanged() {
    let Some(default) = build(None) else {
        eprintln!("SKIP: tsl-013_Al_027-ENDF8.0.endf not present");
        return;
    };
    let same = build(Some(CALCEM_TOL)).expect("build at CALCEM_TOL");
    assert_eq!(default.nxs, same.nxs);
    assert_eq!(default.jxs, same.jxs);
    assert_eq!(
        default.xss, same.xss,
        "thermal_from_mf7 must be the CALCEM_TOL case exactly"
    );

    let fine = build(Some(1.0e-3)).expect("build at tol 0.001");
    assert_eq!(
        default.nxs, fine.nxs,
        "the table's dimensions do not depend on tol"
    );
    let itie = (default.jxs[jxs::ITIE] - 1) as usize;
    let itix = (default.jxs[jxs::ITIX] - 1) as usize;
    let itxe = (default.jxs[jxs::ITXE] - 1) as usize;
    assert_eq!(
        default.xss[itie..itix],
        fine.xss[itie..itix],
        "the incident-energy grid is the caller's and must not depend on tol"
    );
    assert_ne!(
        default.xss[itix..itxe],
        fine.xss[itix..itxe],
        "ITIX is calcem's xsi, so a finer tol must change it"
    );
    let moved = default.xss[itxe..]
        .iter()
        .zip(&fine.xss[itxe..])
        .filter(|(a, b)| a != b)
        .count();
    println!(
        "ITXE words changed by tol 0.05 -> 0.001: {moved} of {}",
        default.xss.len() - itxe
    );
    assert!(
        moved > 0,
        "tol did not reach calcem: the emission block is unchanged"
    );
}
