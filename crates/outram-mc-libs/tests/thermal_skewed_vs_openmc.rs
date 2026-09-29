// SPDX-License-Identifier: GPL-3.0

//! **A skewed (IFENG = 1) S(α,β) ACE table is sampled as OpenMC samples it** —
//! GitHub #365 audit.
//!
//! # What was wrong
//!
//! NJOY's skewed form (`iwt = 0`) gives the two outermost bins on each side
//! 0.1 and 0.4 of an interior bin's probability. `ThermalScattering::from_ace`
//! first sampled it as if every bin were equiprobable; after the first audit
//! chunk it refused it. It is now sampled with a port of OpenMC's
//! `IncoherentInelasticAEDiscrete::sample` (`src/secondary_thermal.cpp`):
//! the skewed bin weights, and `E'` and `mu` interpolated between the
//! bracketing incident tables. The equiprobable form keeps its #188 scheme.
//!
//! # Methodology
//!
//! Table: NJOY2016 H in H2O at 293.6 K with `iwt = 0`, `NIEB = 16`. The deck is
//! the IFENG = 2 one with card 9 set to `222 16 0 0 1 4.0 0`
//! (`verification_and_validation/ace_route_physics/thermal_ifeng2_2026-09-29.md`),
//! built into `target/ace_extra/HH2O_iwt0/tape30`; the test skips when it is
//! absent. Reference: exact sums over OpenMC's reader of the same file
//! (`openmc_inputs/thermal_skewed_reference.py` → `data/thermal_skewed_openmc.csv`).
//! `N = 1e6` draws at each of six energies. Pass: `<E'>`, `<E'^2>` and `<mu>`
//! each lie within 5 sample-sem of the reference.
//!
//! # Results (2026-09-29)
//!
//! Printed by the test and recorded in the #365 thread.

use outram_mc_libs::material::thermal::ThermalScattering;
use std::path::PathBuf;

const N: usize = 1_000_000;

#[test]
fn skewed_h_in_h2o_samples_as_openmc() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let tape = root.join("../../target/ace_extra/HH2O_iwt0/tape30");
    if !tape.is_file() {
        println!("{} absent: skipping (see the module doc)", tape.display());
        return;
    }
    let raw = njoy_outram_park_fork::acer::read::read(&tape).expect("read");
    let th = ThermalScattering::from_ace(&raw, "H in H2O").expect("IFENG = 1 must load");
    let csv =
        root.join("verification_and_validation/ace_route_physics/data/thermal_skewed_openmc.csv");
    let text = std::fs::read_to_string(csv).expect("reference");
    let mut rows = 0;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let v: Vec<f64> = line.split(',').map(|x| x.parse().unwrap()).collect();
        let (e, want_e, want_e2, want_mu) = (v[0], v[1], v[2], v[3]);
        let mut seed = 0x5EE6_0000 + rows as u64;
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
