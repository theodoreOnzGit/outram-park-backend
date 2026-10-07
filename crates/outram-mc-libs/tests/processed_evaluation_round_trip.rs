// SPDX-License-Identifier: GPL-3.0

//! **A nuclide built from shipped numbers is the nuclide built from the tape,
//! bit for bit** (gh:#786).
//!
//! # Methodology
//!
//! The HTR-10 browser demo splits nuclear-data processing over a pool of Web
//! Workers: one worker runs [`Nuclide::process_evaluation`] (RECONR, BROADR,
//! PURR) for a nuclide, sends [`ProcessedEvaluation::to_f64s`] to the others,
//! and each builds the nuclide with [`Nuclide::from_processed`] from its own
//! copy of the tape. Thermal laws travel the same way
//! ([`ThermalScattering::to_f64s`]). This test builds each nuclide twice from
//! `reference-data/endf/` at 300.15 K and tolerance 1e-3 (the HTR-10 record's
//! data settings):
//!
//! - **direct:** [`Nuclide::from_tape`], the path every recorded run took;
//! - **shipped:** `process_evaluation` → `to_f64s` → `from_f64s` →
//!   `from_processed`.
//!
//! and requires, with **no tolerance**: every field of `xs_at_energy` and
//! `total_at_energy` at 1300+ energies from 1e-5 eV to 20 MeV; the URR band
//! sample at 9 band variates over the unresolved range; `total_upper_bound`;
//! 64 draws each of the elastic cosine, the inelastic channel and (fissile
//! nuclides) the fission energy, with the seeds they leave; DBRC and URR
//! presence. The laws: σ and 64 sampled `(E', μ)` per energy, through a
//! nuclide carrying the received law.
//!
//! Nuclides, and what each exercises: He-4 (no resonances), Si-28 (resolved
//! resonances, inelastic levels), U-234 (fissile, an unresolved range, so
//! PURR tables cross the boundary); laws: C-in-SiC from its tape (coherent
//! elastic plus inelastic).
//!
//! **Pass criterion:** identical in every compared value.
//!
//! # Results, 2026-10-07
//!
//! All identical; the counts are printed by the test. (i9-13900K, release,
//! one thread, machine shared with other agents.)

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::nuclide::{MicroXS, Nuclide};
use outram_mc_libs::material::processed::ProcessedEvaluation;
use outram_mc_libs::material::thermal::ThermalScattering;

const TEMP_K: f64 = 300.15;
const TOL: f64 = 1.0e-3;

fn bits(x: &MicroXS) -> [u64; 9] {
    [
        x.total.to_bits(),
        x.elastic.to_bits(),
        x.fission.to_bits(),
        x.absorption.to_bits(),
        x.inelastic.to_bits(),
        x.n2n.to_bits(),
        x.n3n.to_bits(),
        x.mt5.to_bits(),
        x.nu_fission.to_bits(),
    ]
}

fn tape(file: &str) -> Option<(Tape, i32)> {
    let path = reference_endf(file)?;
    let t = Tape::read_file(&path).expect("read tape");
    let mat = *t.materials().first().expect("a material");
    Some((t, mat))
}

/// Both routes for one nuclide; returns the values compared.
fn compare(file: &str, name: &str, fissile: bool) -> Option<usize> {
    let (t, mat) = tape(file)?;
    let direct = Nuclide::from_tape(&t, mat, name, TEMP_K, TOL).expect("direct");
    let words = Nuclide::process_evaluation(&t, mat, TEMP_K, TOL, TOL)
        .expect("process")
        .to_f64s();
    let shipped = ProcessedEvaluation::from_f64s(&words).expect("decode");
    assert_eq!(
        shipped
            .to_f64s()
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>(),
        words.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        "{name}: re-encoding"
    );
    let shipped = Nuclide::from_processed(&t, mat, name, TEMP_K, shipped).expect("assemble");
    let mut n = 0;
    assert_eq!(direct.has_dbrc(), shipped.has_dbrc(), "{name}: DBRC");
    assert_eq!(
        direct.has_urr_probability_tables(),
        shipped.has_urr_probability_tables(),
        "{name}: URR"
    );
    assert_eq!(
        format!("{:?}", direct.urr_range_ev()),
        format!("{:?}", shipped.urr_range_ev()),
        "{name}: URR range"
    );
    let mut es: Vec<f64> = (0..=1200)
        .map(|i| 1.0e-5 * (2.0e7_f64 / 1.0e-5).powf(f64::from(i) / 1200.0))
        .collect();
    if let Some((lo, hi)) = direct.urr_range_ev() {
        es.extend((0..=100).map(|i| lo + (hi - lo) * f64::from(i) / 100.0));
    }
    for &e in &es {
        assert_eq!(
            bits(&direct.xs_at_energy(e, TEMP_K)),
            bits(&shipped.xs_at_energy(e, TEMP_K)),
            "{name}: xs at {e} eV"
        );
        assert_eq!(
            direct.total_at_energy(e, TEMP_K).to_bits(),
            shipped.total_at_energy(e, TEMP_K).to_bits(),
            "{name}: total at {e}"
        );
        assert_eq!(
            direct.total_upper_bound(e, TEMP_K).to_bits(),
            shipped.total_upper_bound(e, TEMP_K).to_bits(),
            "{name}: bound at {e}"
        );
        for xi in [0.0, 0.05, 0.2, 0.33, 0.5, 0.66, 0.8, 0.95, 0.999] {
            assert_eq!(
                format!("{:?}", direct.sample_urr(e, xi)),
                format!("{:?}", shipped.sample_urr(e, xi)),
                "{name}: URR at {e}, {xi}"
            );
        }
        n += 13 + 9;
    }
    for &e in &[0.0253, 1.0, 6.7, 1.0e3, 3.0e4, 1.0e6, 5.0e6] {
        let (mut s1, mut s2) = (17u64, 17u64);
        for _ in 0..64 {
            assert_eq!(
                format!("{:?}", direct.sample_elastic_mu_cm(e, &mut s1)),
                format!("{:?}", shipped.sample_elastic_mu_cm(e, &mut s2)),
                "{name}: mu at {e}"
            );
            assert_eq!(
                format!("{:?}", direct.sample_inelastic(e, &mut s1)),
                format!("{:?}", shipped.sample_inelastic(e, &mut s2)),
                "{name}: inelastic at {e}"
            );
            if fissile {
                assert_eq!(
                    direct.sample_fission_energy(e, &mut s1).to_bits(),
                    shipped.sample_fission_energy(e, &mut s2).to_bits(),
                    "{name}: chi at {e}"
                );
            }
            n += 3;
        }
        assert_eq!(s1, s2, "{name}: seeds diverged at {e}");
    }
    eprintln!(
        "{name}: {} words shipped, {n} values compared, identical",
        words.len()
    );
    Some(n)
}

#[test]
fn shipped_nuclides_are_the_direct_nuclides_bit_for_bit() {
    let mut done = 0;
    for (file, name, fissile) in [
        ("n-002_He_004-ENDF8.0.endf", "He4", false),
        ("n-014_Si_028-ENDF8.0.endf", "Si28", false),
        ("n-092_U_234-ENDF8.0.endf", "U234", true),
    ] {
        match compare(file, name, fissile) {
            Some(_) => done += 1,
            None => eprintln!("SKIP {name}: {file} not in reference-data/endf/"),
        }
    }
    assert!(done > 0 || reference_endf("n-002_He_004-ENDF8.0.endf").is_none());
}

/// A law built from its tape and one received as `f64s` scatter alike, bit
/// for bit, when carried by the same nuclide.
#[test]
fn a_shipped_thermal_law_is_the_built_law_bit_for_bit() {
    let Some(path) = reference_endf("tsl-CinSiC.endf") else {
        eprintln!("SKIP: tsl-CinSiC.endf not in reference-data/endf/");
        return;
    };
    let law =
        ThermalScattering::from_endf_file(path.to_str().expect("utf-8 path"), 44, TEMP_K, "c_SiC")
            .expect("law");
    let back = ThermalScattering::from_f64s(&law.to_f64s()).expect("decode");
    let Some((t, mat)) = tape("n-006_C_012-ENDF8.0.endf") else {
        return;
    };
    let c12 = Nuclide::from_tape(&t, mat, "C12", TEMP_K, TOL).expect("C12");
    let (a, b) = (
        c12.clone().with_thermal_scattering(law),
        c12.with_thermal_scattering(back),
    );
    let mut n = 0;
    for i in 0..=400 {
        let e = 1.0e-5 * (5.0_f64 / 1.0e-5).powf(f64::from(i) / 400.0);
        assert_eq!(
            bits(&a.xs_at_energy(e, TEMP_K)),
            bits(&b.xs_at_energy(e, TEMP_K)),
            "xs at {e}"
        );
        let (mut s1, mut s2) = (5u64 + i as u64, 5u64 + i as u64);
        for _ in 0..64 {
            assert_eq!(
                format!("{:?}", a.sample_thermal(e, &mut s1)),
                format!("{:?}", b.sample_thermal(e, &mut s2)),
                "sample at {e}"
            );
        }
        n += 65;
    }
    eprintln!("C-in-SiC: {n} values compared, identical");
}
