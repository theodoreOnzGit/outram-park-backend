// SPDX-License-Identifier: GPL-3.0

//! **Reading a thermal S(α,β) ACE table back** — ACE gap 2.
//!
//! This crate wrote thermal ACE byte-for-byte against NJOY2016 but had no
//! reader, so `outram_mc_libs::ThermalScattering` could only be built from a
//! `tsl-*.endf` tape or by regenerating the law with LEAPR. A thermal lattice
//! assembled from an ACE library had **no bound-atom scattering at all**.
//!
//! # Why a round trip is strong evidence HERE, unusually
//!
//! A round trip through one crate's own writer and reader normally proves only
//! that the two halves agree, and this workspace has said so repeatedly. The
//! difference in this case is that the **writer's output is independently
//! established as byte-identical to NJOY2016's** —
//! `verification_and_validation/acer_thermal_vs_njoy2016.md` and the IFENG=1/2
//! record beside it, over all nine TSL tapes. So a file this writer produces
//! *is* NJOY's file, and reading it back is reading NJOY's.
//!
//! What a round trip still cannot catch is a convention both halves share and
//! upstream does not. That is why the block layout was taken from
//! `openmc/data/thermal.py::ThermalScattering.from_ace` rather than from our own
//! writer, and why the stride (`n_mu + 2` there, `1 + n_mu` here) is called out
//! in `acer::thermal_read`'s docs.
//!
//! # Results, 2026-09-25
//!
//! Printed by the test. Fixture: `tsl-013_Al_027-ENDF8.0.endf` (MAT=53), the
//! same Al-27 the existing thermal ACE tests use, so the two describe one case.

use std::fs::File;

use njoy_outram_park_fork::acer::thermal::ThermalAceOptions;
use njoy_outram_park_fork::acer::thermal_read::{decode_thermal, AceThermalElastic};
use njoy_outram_park_fork::acer::AceTable;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::thermr::mf7::parse_mf7;

const AL_MAT: i32 = 53;
const N_OUTGOING: usize = 16;
const N_COSINES: usize = 8;

fn build() -> Option<(AceTable, Vec<f64>)> {
    let p = njoy_outram_park_fork::reference_data::reference_endf_dir()
        .join("tsl-013_Al_027-ENDF8.0.endf");
    let tape = Tape::read(File::open(p).ok()?).ok()?;
    let mf7 = parse_mf7(&tape, AL_MAT).ok()?;
    let ne = 40;
    let (elo, ehi) = (1.0e-4_f64, 4.0_f64);
    let grid: Vec<f64> = (0..ne)
        .map(|i| elo * (ehi / elo).powf(i as f64 / (ne - 1) as f64))
        .collect();
    let temp = mf7.incoherent_inelastic.as_ref()?.temperature_k;
    let opts = ThermalAceOptions {
        n_outgoing: N_OUTGOING,
        n_cosines: N_COSINES,
        natom: 1.0,
        emax_ev: 4.0,
        ..Default::default()
    };
    let t = AceTable::thermal_from_mf7(&mf7, temp, "al27", 0, &grid, opts).ok()?;
    Some((t, grid))
}

/// **THE GATE: the reader recovers what the writer put in.**
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "builds a thermal ACE table from the Al-27 TSL tape; runs by default"
)]
fn a_thermal_table_we_wrote_reads_back() {
    let Some((built, grid)) = build() else {
        eprintln!("SKIP: tsl-013_Al_027-ENDF8.0.endf not present");
        return;
    };
    // Through the FILE, not in memory: the Type-1 text is the format under test,
    // and the writer's byte parity against NJOY is what makes this evidence.
    let mut buf: Vec<u8> = Vec::new();
    built.write_to(&mut buf).expect("write Type-1");
    let raw = njoy_outram_park_fork::acer::read::parse_type1(
        &String::from_utf8(buf).expect("Type-1 is ASCII"),
    )
    .expect("our own Type-1 output must parse");

    let t = decode_thermal(&raw).expect("a table this crate wrote must decode");

    println!(
        "Al-27 thermal: kT = {:.6e} eV, IFENG = {}, {} incident energies, \
         {} bins x {} cosines",
        t.kt_ev,
        t.ifeng,
        t.inel_energy.len(),
        t.emission[0].e_out.len(),
        t.emission[0].n_mu
    );

    // The incident grid is the one handed to the writer.
    assert_eq!(
        t.inel_energy.len(),
        grid.len(),
        "the reader recovered {} incident energies from a {}-point grid",
        t.inel_energy.len(),
        grid.len()
    );
    for (i, (&got, &want)) in t.inel_energy.iter().zip(&grid).enumerate() {
        assert!(
            (got - want).abs() <= 1.0e-6 * want,
            "incident energy {i}: {got:.9e} eV read back against {want:.9e} written. \
             ACE stores MeV, so exact equality is not available, but 1e-6 relative \
             is far looser than the format's precision and a miss here means the \
             ITIE grid is being read at the wrong offset"
        );
    }
    assert!(
        t.inel_energy.windows(2).all(|w| w[1] > w[0]),
        "ascending grid"
    );

    // Cross sections must be positive and finite -- a zero sigma_inel across the
    // board is what a wrong ITIX offset looks like, and it would remove thermal
    // scattering from the lattice without erroring.
    assert_eq!(t.inel_xs.len(), t.inel_energy.len());
    let n_positive = t.inel_xs.iter().filter(|&&x| x > 0.0).count();
    println!(
        "  sigma_inel: {n_positive}/{} points positive, range {:.4} .. {:.4} b",
        t.inel_xs.len(),
        t.inel_xs.iter().cloned().fold(f64::INFINITY, f64::min),
        t.inel_xs.iter().cloned().fold(0.0, f64::max)
    );
    assert!(
        n_positive > t.inel_xs.len() / 2,
        "only {n_positive} of {} inelastic cross sections are positive; a wrong \
         ITIX offset reads zeros or neighbouring block data",
        t.inel_xs.len()
    );
    assert!(
        t.inel_xs.iter().all(|x| x.is_finite() && *x >= 0.0),
        "every sigma_inel must be finite and non-negative"
    );

    // One emission table per incident energy, each the declared shape.
    assert_eq!(t.emission.len(), t.inel_energy.len());
    for (i, e) in t.emission.iter().enumerate() {
        assert_eq!(
            e.e_out.len(),
            N_OUTGOING,
            "energy {i}: {} outgoing bins, NIEB said {N_OUTGOING}",
            e.e_out.len()
        );
        assert_eq!(e.n_mu, N_COSINES, "energy {i}: {} cosines", e.n_mu);
        assert_eq!(e.cosines.len(), N_OUTGOING * N_COSINES);
        assert!(
            e.e_out.iter().all(|&x| x > 0.0 && x.is_finite()),
            "energy {i}: an outgoing energy is non-positive, which means the \
             stride through ITXE is wrong -- a cosine has been read as an energy"
        );
        // **The sharpest structural check.** Cosines live in [-1, 1]; outgoing
        // energies are eV and run to ~1e0. If the stride were off by one, a
        // cosine would land in e_out (caught above) and an ENERGY would land
        // here, out of range by orders of magnitude.
        assert!(
            e.cosines.iter().all(|&m| (-1.0001..=1.0001).contains(&m)),
            "energy {i}: a cosine is outside [-1, 1]; an outgoing ENERGY has been \
             read into the cosine slots, i.e. the 1 + n_mu stride is wrong"
        );
    }

    // Al-27 is crystalline, so it must have COHERENT elastic -- dropping the
    // elastic channel silently would remove real scattering from a lattice.
    match &t.elastic {
        AceThermalElastic::Coherent { energy, cumulative } => {
            println!(
                "  coherent elastic: {} Bragg edges, {:.4e} .. {:.4e} eV",
                energy.len(),
                energy.first().copied().unwrap_or(0.0),
                energy.last().copied().unwrap_or(0.0)
            );
            assert!(!energy.is_empty() && energy.len() == cumulative.len());
            assert!(
                energy.windows(2).all(|w| w[1] > w[0]),
                "Bragg edges must be ascending"
            );
            assert!(
                cumulative.windows(2).all(|w| w[1] >= w[0]),
                "the CUMULATIVE structure factor must be non-decreasing; a \
                 decrease means it is being read as a per-edge value"
            );
        }
        other => panic!(
            "Al-27 is crystalline and its ACE table carries coherent elastic, but \
             the reader returned {other:?}"
        ),
    }
}

/// **IFENG = 2 is refused, not silently resampled.**
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "builds a continuous-emission thermal table; runs by default"
)]
fn a_continuous_table_is_refused_with_its_form_named() {
    let p = njoy_outram_park_fork::reference_data::reference_endf_dir()
        .join("tsl-013_Al_027-ENDF8.0.endf");
    let Ok(f) = File::open(p) else {
        eprintln!("SKIP: Al-27 TSL tape not present");
        return;
    };
    let tape = Tape::read(f).expect("parse");
    let mf7 = parse_mf7(&tape, AL_MAT).expect("MF=7");
    let ne = 20;
    let grid: Vec<f64> = (0..ne)
        .map(|i| 1.0e-4 * (4.0_f64 / 1.0e-4).powf(i as f64 / (ne - 1) as f64))
        .collect();
    let temp = mf7.incoherent_inelastic.as_ref().unwrap().temperature_k;
    let opts = ThermalAceOptions {
        n_outgoing: 16,
        n_cosines: 8,
        natom: 1.0,
        emax_ev: 4.0,
        form: njoy_outram_park_fork::acer::thermal::InelasticForm::Continuous,
        ..Default::default()
    };
    let Ok(built) = AceTable::thermal_from_mf7(&mf7, temp, "al27", 0, &grid, opts) else {
        eprintln!("SKIP: the writer declined IFENG=2 for this fixture");
        return;
    };
    let mut buf: Vec<u8> = Vec::new();
    built.write_to(&mut buf).expect("write Type-1");
    let raw = njoy_outram_park_fork::acer::read::parse_type1(
        &String::from_utf8(buf).expect("ASCII"),
    )
    .expect("parse");

    let err = decode_thermal(&raw)
        .expect_err("IFENG=2 must be refused -- the discrete representation has no form for it");
    let m = format!("{err}");
    println!("refused: {m}");
    assert!(m.contains("IFENG = 2"), "the error must name the form: {m}");
    assert!(
        m.contains("resample"),
        "and say what the harm would be, since a silent resample looks like data: {m}"
    );
}
