// SPDX-License-Identifier: GPL-3.0

//! **Discrete lines in ACE tabulated energy laws (INTT = 10·ND + LEP) are read
//! and sampled as OpenMC samples them** — GitHub #365 audit.
//!
//! # What was missing
//!
//! A LAW=4/44/61 table whose leading points are discrete lines was refused by
//! name. On the ENDF side, an MF=6 LAW=1 section with `ND > 0` gave no law at
//! all. `ChiEout::n_discrete` now carries the lines, and the sampler follows
//! OpenMC's `ContinuousTabular::sample` (`src/distribution_energy.cpp`):
//! - a line is emitted at its own energy, **unscaled**;
//! - the continuum envelope starts at `e_out[n_discrete]`.
//!
//! # Methodology
//!
//! No neutron evaluation in ENDF/B-VIII.0 has discrete lines on an MF=6 LAW=1
//! neutron subsection. The scan on 2026-09-29 covered 2377 such subsections in
//! 557 tapes. So no NJOY-built table exercises the form.
//! `verification_and_validation/ace_route_physics/openmc_inputs/discrete_lines_reference.py`
//! therefore **constructs** one. It takes NJOY2016 Li-7 (ENDF/B-VIII.0), whose
//! MT=16 is LAW=4, and turns the first point of every MT=16 table into a line
//! carrying the mass of the first panel, writing
//! `target/ace_extra/Li7_lines.ace`. OpenMC's reader of that file sees the
//! lines (a `Mixture` per table). The exact mean of OpenMC's sampling scheme at
//! five energies is written to `data/discrete_lines_openmc.csv`.
//!
//! Pass: the mean of `N = 1e6` MT=16 emission energies from
//! `Nuclide::sample_inelastic_emission` lies within 5 sample-sem of that
//! reference. The test skips when the constructed file is absent.
//!
//! # Results (2026-09-29)
//!
//! Printed by the test and recorded in the #365 thread.

use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::Nuclide;
use std::path::PathBuf;

#[test]
fn discrete_lines_sample_as_openmc() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ace = root.join("../../target/ace_extra/Li7_lines.ace");
    if !ace.is_file() {
        println!("{} absent: skipping", ace.display());
        return;
    }
    let nuc = Nuclide::from_ace_file(&ace, "Li7").expect("a table with discrete lines must load");
    let csv =
        root.join("verification_and_validation/ace_route_physics/data/discrete_lines_openmc.csv");
    let text = std::fs::read_to_string(csv).expect("reference");
    let u = Direction::new(0.0, 0.0, 1.0);
    const N: usize = 1_000_000;
    let mut rows = 0;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let v: Vec<f64> = line.split(',').map(|x| x.parse().unwrap()).collect();
        let (e, want) = (v[0], v[1]);
        let mut seed = 0xD15C_0000 + rows as u64;
        let xs: Vec<f64> = (0..N)
            .map(|_| nuc.sample_inelastic_emission(16, e, u, 0.0, &mut seed).0)
            .collect();
        let m = xs.iter().sum::<f64>() / N as f64;
        let var = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (N - 1) as f64;
        let z = (m - want) / (var / N as f64).sqrt();
        println!(
            "E = {:.2} MeV: <E'> {:.2} keV (OpenMC {:.2} keV), z {z:+.2}",
            e / 1e6,
            m / 1e3,
            want / 1e3
        );
        assert!(z.abs() < 5.0, "E = {e}: z = {z}");
        rows += 1;
    }
    assert_eq!(rows, 5);
}
