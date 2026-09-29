// SPDX-License-Identifier: GPL-3.0

//! **A continuous (IFENG = 2) S(α,β) ACE table is read and sampled as OpenMC
//! samples it** — GitHub #365 audit.
//!
//! # What was missing
//!
//! `ThermalScattering::from_ace` refused IFENG = 2, the form OpenMC's own
//! libraries use (NJOY `iwt = 2`). It now decodes it
//! (`acer::thermal_read::AceThermalContinuous`) and samples it with a port of
//! OpenMC's `IncoherentInelasticAE::sample` (`src/secondary_thermal.cpp`):
//! nearer incident table, lin-lin cdf inversion, the energy shift to the
//! actual incident energy, and an interpolated, smeared equiprobable cosine.
//! The binned forms keep their own scheme (#188).
//!
//! # Methodology
//!
//! Table: NJOY2016 H in H2O at 293.6 K with `iwt = 2`. The deck is in
//! `verification_and_validation/ace_route_physics/thermal_ifeng2_2026-09-29.md`
//! and the table is built into `target/ace_extra/HH2O_iwt2/tape30`
//! (regenerable); the test skips when it is absent.
//!
//! Reference: OpenMC's `ThermalScattering.from_ace` reader of the same file.
//! OpenMC's sampling is written as a deterministic function of its uniform and
//! integrated with a 400 000-point midpoint rule
//! (`openmc_inputs/thermal_ifeng2_reference.py`, into
//! `data/thermal_ifeng2_openmc.csv`), which gives the exact `<E'>`, `<E'^2>`
//! and `<mu>` at six incident energies from 5 meV to 2.5 eV.
//!
//! outram-mc draws `N = 1 000 000` samples at each energy. Pass: each of the
//! three sample means lies within **5 sigma** (its own sample sem) of the
//! reference.
//!
//! # Results (2026-09-29)
//!
//! Printed by the test and recorded in the V&V note.

use outram_mc_libs::material::thermal::ThermalScattering;
use std::path::PathBuf;

const N: usize = 1_000_000;

#[test]
fn ifeng2_h_in_h2o_samples_as_openmc() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let tape = root.join("../../target/ace_extra/HH2O_iwt2/tape30");
    if !tape.is_file() {
        println!("{} absent: skipping (see the module doc)", tape.display());
        return;
    }
    let raw = njoy_outram_park_fork::acer::read::read(&tape).expect("read");
    let th = ThermalScattering::from_ace(&raw, "H in H2O").expect("IFENG = 2 must load");
    let csv =
        root.join("verification_and_validation/ace_route_physics/data/thermal_ifeng2_openmc.csv");
    let text = std::fs::read_to_string(csv).expect("reference");
    let mut rows = 0;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let v: Vec<f64> = line.split(',').map(|x| x.parse().unwrap()).collect();
        let (e, want_e, want_e2, want_mu) = (v[0], v[1], v[2], v[3]);
        let mut seed = 0x1FE6_0000 + rows as u64;
        let draws: Vec<(f64, f64)> = (0..N)
            .map(|_| th.sample(e, &mut seed).expect("below cutoff"))
            .collect();
        let stat = |f: &dyn Fn(&(f64, f64)) -> f64| {
            let m = draws.iter().map(f).sum::<f64>() / N as f64;
            let var = draws.iter().map(|d| (f(d) - m).powi(2)).sum::<f64>() / (N - 1) as f64;
            (m, (var / N as f64).sqrt())
        };
        let (me, se) = stat(&|d| d.0);
        let (me2, se2) = stat(&|d| d.0 * d.0);
        let (mm, sm) = stat(&|d| d.1);
        let (z1, z2, z3) = (
            (me - want_e) / se,
            (me2 - want_e2) / se2,
            (mm - want_mu) / sm,
        );
        println!(
            "E = {e:.4} eV: <E'> {me:.6} (OpenMC {want_e:.6}, z {z1:+.2}); <E'^2> z {z2:+.2}; <mu> {mm:.5} (OpenMC {want_mu:.5}, z {z3:+.2})"
        );
        assert!(
            z1.abs() < 5.0 && z2.abs() < 5.0 && z3.abs() < 5.0,
            "E = {e}: z = {z1}, {z2}, {z3}"
        );
        rows += 1;
    }
    assert_eq!(rows, 6);
}
