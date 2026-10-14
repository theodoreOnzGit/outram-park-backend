//! **Thermal S(α,β) ACE tables at temperatures other than the evaluation's
//! base temperature are NJOY2016's, word for word**, and the temperatures
//! NJOY2016 refuses are refused here too (GitHub #866, first checkbox).
//!
//! # Methodology
//!
//! Same instrument as `tests/acer_thermal_word_parity.rs` (GitHub #865), at
//! temperatures that are not the thermal evaluation's base temperature `T₀`.
//! Each case runs NJOY2016 2016.79 (`ac5adf5`) on the deck committed in
//! `reference-data/acer/thermal_decks/`: `reconr` (0.001) → `broadr` (the
//! requested temperature) → `thermr` (16 cosines, `iform = 0`, `tol = 0.001`,
//! `emax = 4 eV`) → `acer` (`iopt = 2`, 16 equiprobable outgoing energies,
//! `iwt = 1`), every unit positive. This crate builds the same table:
//! RECONR + BROADR of the same incident tape to the same temperature,
//! `parse_mf7_at_temperature` of the thermal tape at that temperature, and
//! `AceTable::thermal_from_mf7_on_pendf`. Compared by `acer::compare` (header,
//! NXS, JXS, every XSS word) and as text byte for byte, the date and comment
//! excepted.
//!
//! | case | T (K) | `T₀` (K) | on the MF=7 tape? | NJOY2016 |
//! |---|---|---|---|---|
//! | crystalline graphite (MAT 30) on C-12 (625) | 600 | 296 | yes (inelastic and Bragg) | table |
//! | crystalline graphite | 600.5 | 296 | within `calcem`'s `T/500` of 600 | table, "difference ... 5.00E-01" |
//! | H in H₂O (MAT 1) on H-1 (125), natom 2 | 293.6 | 283.6 | yes | table |
//! | H in H₂O | 293.9 | 283.6 | within `T/500` of 293.6 | table, "difference ... 3.00E-01" |
//! | crystalline graphite | 450 | 296 | no (between 400 and 500) | refuses |
//! | crystalline graphite | 603 | 296 | no (3 K from 600: inside `rdelas`'s `T/1000+5`, outside `calcem`'s `T/500`) | refuses |
//! | H in H₂O | 330 | 283.6 | no (between 323.6 and 350) | refuses |
//!
//! What NJOY2016 does with a temperature (read from `thermr.f90`):
//! - `calcem` walks MF=7/MT=4's temperature blocks and stops at the first
//!   with `abs(t-temp) <= temp/500`; a block above the request first, or
//!   running out, is "desired temperature not found" (`thermr.f90:1720-1741`).
//!   A match that is not exact prints "difference between temperatures
//!   desired and found" and goes on: `S(α,β)` and `kT` are the tabulated
//!   temperature's (`tev=t*bk` with `t` left at the tape's value,
//!   `thermr.f90:1924`), `T_eff` the requested one's (`tempt=temp`,
//!   `:1812-1834`).
//! - `rdelas` keeps the Bragg `S(E)` of the last tabulated temperature with
//!   `abs(tnow-temp) < temp/1000+5` (`thermr.f90:540-555`), and never
//!   refuses: with no match it keeps the base temperature's. MT=4 is always
//!   read on this path, so `calcem` refuses first whenever `rdelas` would
//!   find nothing.
//! - Nothing in THERMR interpolates `S(α,β)` or `S(E)` in temperature.
//!
//! Pass criterion: identical in every compared word and every byte outside
//! the date and comment fields; the three refused temperatures return an
//! error. No tolerance.
//!
//! # Results (2026-10-14)
//!
//! See `verification_and_validation/acer_thermal_temperatures_2026_10_14.md`.
//! The four tables are identical to NJOY2016's in every compared word and in
//! every byte outside the date and comment. NJOY2016 stops in `calcem` on
//! the three refused decks; so does this crate. Before this change the port
//! built a table at 450 K (measured: this test failed there) by `LI`-law
//! interpolation; by `select_temperature`'s rule it would have interpolated
//! 330 K too and snapped 603 K to 600 K (read, not run).

use njoy_outram_park_fork::acer::compare::compare;
use njoy_outram_park_fork::acer::read::{self, AceFileType, RawAceTable};
use njoy_outram_park_fork::acer::thermal::{
    ace_incident_grid, IncidentPendf, InelasticForm, ThermalAceOptions,
};
use njoy_outram_park_fork::acer::AceTable;
use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{reference_data_dir, reference_endf};
use njoy_outram_park_fork::thermr::mf7::parse_mf7_at_temperature;
use njoy_outram_park_fork::NjoyError;

struct Case {
    incident: &'static str,
    mat: i32,
    tsl: &'static str,
    tmat: i32,
    temp_k: f64,
    natom: f64,
    name: &'static str,
    /// ACER card 8a in the deck.
    za: i32,
}

const GRAPHITE: Case = Case {
    incident: "n-006_C_012-ENDF8.0.endf",
    mat: 625,
    tsl: "tsl-crystalline-graphite.endf",
    tmat: 30,
    temp_k: 296.0,
    natom: 1.0,
    name: "grph",
    za: 6012,
};

const H_IN_H2O: Case = Case {
    incident: "n-001_H_001-ENDF8.0-Beta6.endf",
    mat: 125,
    tsl: "tsl-HinH2O.endf",
    tmat: 1,
    temp_k: 283.6,
    natom: 2.0,
    name: "hh2o",
    za: 1001,
};

/// Build the table the deck builds, at `c.temp_k`.
fn build(c: &Case) -> Result<AceTable, NjoyError> {
    let inc_tape = Tape::read_file(&reference_endf(c.incident).expect("incident tape")).unwrap();
    let r0 = reconr(
        &inc_tape,
        &ReconrConfig {
            mat: c.mat,
            tolerance: 1.0e-3,
            temperature: 0.0,
        },
    )
    .unwrap();
    let r = broaden_result(&r0, c.temp_k);
    let elastic = r
        .sections
        .iter()
        .find(|s| s.mt.number() == 2)
        .expect("MT=2")
        .pairs
        .clone();
    let incident = IncidentPendf {
        mat: c.mat,
        awr: r.material.awr,
        moderator_za: vec![c.za],
        elastic,
    };
    let tsl = Tape::read_file(&reference_endf(c.tsl).expect("thermal tape")).unwrap();
    let mf7 = parse_mf7_at_temperature(&tsl, c.tmat, Some(c.temp_k))?;
    let grid = ace_incident_grid(4.0);
    let opts = ThermalAceOptions {
        n_outgoing: 16,
        n_cosines: 16,
        emax_ev: 4.0,
        natom: c.natom,
        form: InelasticForm::Equiprobable,
    };
    AceTable::thermal_from_mf7_on_pendf(&mf7, c.temp_k, c.name, 0, &grid, opts, 1.0e-3, &incident)
}

fn check(njoy_file: &str, c: Case) {
    let path = reference_data_dir("acer").join(njoy_file);
    let bytes = std::fs::read(&path).expect("reference table");
    let njoy: RawAceTable = read::read_library_from_bytes(bytes.clone(), njoy_file)
        .expect("parse")
        .pop()
        .unwrap();
    let ace = build(&c).unwrap();
    let ours_text = ace
        .to_raw(AceFileType::Type1Ascii)
        .unwrap()
        .to_type1_string();
    let ours = read::parse_type1(&ours_text).unwrap();
    let njoy_text = String::from_utf8(read::inflate_gzip(&bytes, &path).unwrap()).unwrap();
    let (ol, nl): (Vec<&str>, Vec<&str>) =
        (ours_text.lines().collect(), njoy_text.lines().collect());
    assert_eq!(ol.len(), nl.len(), "{njoy_file}: line count");
    assert_eq!(
        ol[0][..35],
        nl[0][..35],
        "{njoy_file}: ZAID, AWR and kT as text"
    );
    assert_eq!(
        ol[1][70..],
        nl[1][70..],
        "{njoy_file}: the material tag as text"
    );
    if let Some(k) = (2..ol.len()).find(|&k| ol[k] != nl[k]) {
        panic!(
            "{njoy_file}: line {} differs:\n ours {:?}\n njoy {:?}",
            k + 1,
            ol[k],
            nl[k]
        );
    }
    let cmp = compare(&ours, &njoy);
    let lines = cmp.report_lines();
    println!("{njoy_file}:");
    for l in &lines {
        println!("  {l}");
    }
    assert!(
        cmp.identical(),
        "{njoy_file} differs from NJOY2016: {lines:#?}"
    );
}

#[test]
fn crystalline_graphite_at_600k_is_njoy2016s_table() {
    check(
        "graphite_600k_thermal_tol0p001_njoy2016.ace.gz",
        Case {
            temp_k: 600.0,
            ..GRAPHITE
        },
    );
}

#[test]
fn crystalline_graphite_at_600p5k_is_njoy2016s_table() {
    check(
        "graphite_600p5k_thermal_tol0p001_njoy2016.ace.gz",
        Case {
            temp_k: 600.5,
            ..GRAPHITE
        },
    );
}

#[test]
fn h_in_h2o_at_293p6k_is_njoy2016s_table() {
    check(
        "hh2o_293p6k_thermal_tol0p001_njoy2016.ace.gz",
        Case {
            temp_k: 293.6,
            ..H_IN_H2O
        },
    );
}

#[test]
fn h_in_h2o_at_293p9k_is_njoy2016s_table() {
    check(
        "hh2o_293p9k_thermal_tol0p001_njoy2016.ace.gz",
        Case {
            temp_k: 293.9,
            ..H_IN_H2O
        },
    );
}

/// `calcem` reads `T_eff` at the requested temperature and `kT` at the
/// tabulated one (`thermr.f90:1812-1834, 1924`). Neither 600.5 K graphite nor
/// 293.9 K water moves a printed word through it (NJOY2016's own tables at
/// 600/600.5 K and 293.6/293.9 K differ only in the header's kT), so this
/// pins that the request reaches the short-collision-time kernel at all:
/// far outside graphite's alpha grid the kernel
/// moves with `teff_request_k`, and is unchanged when it equals the
/// tabulated temperature.
#[test]
fn effective_temperature_is_read_at_the_request() {
    let tsl =
        Tape::read_file(&reference_endf("tsl-crystalline-graphite.endf").expect("thermal tape"))
            .unwrap();
    let ii = parse_mf7_at_temperature(&tsl, 30, Some(600.0))
        .unwrap()
        .incoherent_inelastic
        .unwrap();
    assert_eq!(ii.temperature_k, 600.0);
    assert_eq!(ii.teff_request_k, None);
    // Elastic-like backscatter at E = E' with alpha = 4E/(A kT0) four times
    // the table's last alpha (LAT = 1): the short-collision-time kernel.
    let alpha_max = *ii.s_tables[0].alpha.last().unwrap();
    let e = alpha_max * ii.mass_ratio() * 0.0253;
    let at = |req: Option<f64>| {
        let k = njoy_outram_park_fork::thermr::mf7::IncoherentInelastic {
            teff_request_k: req,
            ..ii.clone()
        };
        k.double_differential(e, e, -1.0, 600.0, 1.0)
    };
    let base = at(None);
    assert!(base > 0.0, "the SCT kernel is reached: {base}");
    assert_eq!(at(Some(600.0)), base);
    let moved = at(Some(600.5));
    println!(
        "SCT kernel at 600 K: {base:e}; T_eff read at 600.5 K: {moved:e} ({:+e} relative)",
        moved / base - 1.0
    );
    assert_ne!(moved, base);
}

/// NJOY2016 stops in `calcem` with "desired temperature not found" on all
/// three decks (`graphite_450k_tol0p001.njoy`, `graphite_603k_tol0p001.njoy`,
/// `hh2o_330k_tol0p001.njoy`, run 2026-10-14).
#[test]
fn temperatures_njoy2016_refuses_are_refused() {
    for c in [
        Case {
            temp_k: 450.0,
            ..GRAPHITE
        },
        Case {
            temp_k: 603.0,
            ..GRAPHITE
        },
        Case {
            temp_k: 330.0,
            ..H_IN_H2O
        },
    ] {
        match build(&c) {
            Err(NjoyError::TemperatureNotTabulated { requested_k, .. }) => {
                assert_eq!(requested_k, c.temp_k)
            }
            Err(e) => panic!("{} at {} K: refused, but with {e:?}", c.name, c.temp_k),
            Ok(_) => panic!(
                "{} at {} K: built a table NJOY2016 refuses to build",
                c.name, c.temp_k
            ),
        }
    }
}

/// An `Mf7` resolved at another temperature than the one `calcem` reads is
/// refused, not built into a mislabelled table: `parse_mf7` gives graphite's
/// 296 K `S(alpha,beta)`, and a 600 K table from it would carry 296 K's
/// inelastic data under 600 K's header. The check comes before any of the
/// incident data is used, so a two-point elastic stands in for the PENDF.
#[test]
fn s_alpha_beta_at_another_temperature_is_refused() {
    let tsl =
        Tape::read_file(&reference_endf("tsl-crystalline-graphite.endf").expect("thermal tape"))
            .unwrap();
    let mf7 = njoy_outram_park_fork::thermr::mf7::parse_mf7(&tsl, 30).unwrap();
    let incident = IncidentPendf {
        mat: 625,
        awr: 11.8969,
        moderator_za: vec![6012],
        elastic: vec![(1.0e-5, 4.7), (4.0, 4.7)],
    };
    let opts = ThermalAceOptions {
        n_outgoing: 16,
        n_cosines: 16,
        emax_ev: 4.0,
        natom: 1.0,
        form: InelasticForm::Equiprobable,
    };
    match AceTable::thermal_from_mf7_on_pendf(
        &mf7,
        600.0,
        "grph",
        0,
        &ace_incident_grid(4.0),
        opts,
        1.0e-3,
        &incident,
    ) {
        Err(NjoyError::ThermalTemperatureMismatch {
            requested_k,
            calcem_k,
            resolved_k,
            interpolated,
        }) => {
            assert_eq!(
                (requested_k, calcem_k, resolved_k, interpolated),
                (600.0, 600.0, 296.0, false)
            );
        }
        other => panic!(
            "expected ThermalTemperatureMismatch, got {:?}",
            other.map(|_| "a table")
        ),
    }
}
