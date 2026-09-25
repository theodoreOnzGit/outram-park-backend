// SPDX-License-Identifier: GPL-3.0

//! **`ThermalScattering` from an ACE table** — ACE gap 2, transport side.
//!
//! `ThermalScattering` had two ways in: a `tsl-*.endf` tape, or regenerating the
//! law with LEAPR. So a thermal lattice assembled from an **ACE** library had no
//! bound-atom scattering at all, even though this workspace writes thermal ACE
//! byte-for-byte against NJOY2016.
//!
//! This checks the third way in end to end: ENDF → our thermal ACE writer →
//! `acer::thermal_read` → `ThermalScattering`, and compares the result against
//! the **same tape read directly**, which is the oracle that matters — the two
//! routes must describe one scatterer.
//!
//! # Results, 2026-09-25
//!
//! Printed by the test.

use std::fs::File;

use njoy_outram_park_fork::acer::thermal::ThermalAceOptions;
use njoy_outram_park_fork::acer::AceTable;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::thermr::mf7::parse_mf7;
use outram_mc_libs::material::thermal::ThermalScattering;

const AL_MAT: i32 = 53;

#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "builds a thermal ACE table from the Al-27 TSL tape; runs by default"
)]
fn a_thermal_scatterer_builds_from_ace_and_matches_the_tape() {
    let p = njoy_outram_park_fork::reference_data::reference_endf_dir()
        .join("tsl-013_Al_027-ENDF8.0.endf");
    let Ok(f) = File::open(&p) else {
        eprintln!("SKIP: tsl-013_Al_027-ENDF8.0.endf not present");
        return;
    };
    let tape = Tape::read(f).expect("parse the TSL tape");
    let mf7 = parse_mf7(&tape, AL_MAT).expect("MF=7");
    let temp = mf7.incoherent_inelastic.as_ref().unwrap().temperature_k;

    let ne = 40;
    let grid: Vec<f64> = (0..ne)
        .map(|i| 1.0e-4 * (4.0_f64 / 1.0e-4).powf(i as f64 / (ne - 1) as f64))
        .collect();
    let built = AceTable::thermal_from_mf7(
        &mf7,
        temp,
        "al27",
        0,
        &grid,
        ThermalAceOptions {
            n_outgoing: 16,
            n_cosines: 8,
            natom: 1.0,
            emax_ev: 4.0,
            ..Default::default()
        },
    )
    .expect("build the thermal ACE table");

    let mut buf: Vec<u8> = Vec::new();
    built.write_to(&mut buf).expect("write Type-1");
    let raw = njoy_outram_park_fork::acer::read::parse_type1(
        &String::from_utf8(buf).expect("ASCII"),
    )
    .expect("parse our own Type-1");

    let from_ace =
        ThermalScattering::from_ace(&raw, "Al in Al27").expect("build from the ACE table");

    println!(
        "from ACE: cutoff {:.4} eV, T = {:.2} K",
        from_ace.cutoff_ev(),
        from_ace.selected_temperature_k()
    );
    assert!(
        (from_ace.selected_temperature_k() - temp).abs() < 1.0,
        "the ACE table's own temperature ({:.3} K) must come back as the \
         scatterer's, against the tape's {temp:.3} K",
        from_ace.selected_temperature_k()
    );
    assert!(
        from_ace.cutoff_ev() > 1.0,
        "the cutoff is the top of the incident grid, ~4 eV here, got {:.4}",
        from_ace.cutoff_ev()
    );

    // **The oracle: the same tape read directly.** Both routes describe one
    // scatterer at one temperature, so the inelastic cross section must agree.
    // The ACE route's grid is the 40 points handed to the writer while the tape
    // route builds its own, so this compares sigma at shared energies rather
    // than arrays.
    let direct = ThermalScattering::from_tape(&tape, AL_MAT, temp, "Al in Al27")
        .expect("build from the tape directly");

    let mut worst = 0.0f64;
    let mut at = 0.0f64;
    for &e in &grid {
        if e >= from_ace.cutoff_ev() || e >= direct.cutoff_ev() {
            continue;
        }
        let (a, b) = (from_ace.total_xs(e), direct.total_xs(e));
        if b <= 0.0 {
            continue;
        }
        let rel = (a - b).abs() / b;
        if rel > worst {
            worst = rel;
            at = e;
        }
    }
    println!("  worst |diff| in total sigma vs the tape route: {:.3} % at {at:.4e} eV", 100.0 * worst);
    assert!(
        worst < 0.05,
        "the ACE route and the tape route disagree by {:.2} % at {at:.4e} eV on one \
         scatterer's total thermal cross section. The ACE table is built FROM this \
         tape, so a disagreement this large means the reader is mis-assembling a \
         block rather than reflecting a discretisation difference",
        100.0 * worst
    );
}
