// SPDX-License-Identifier: GPL-3.0

//! **`Nuclide::sample_fission_neutron_openmc` draws in OpenMC's order** (gh:#1386,
//! #1444 d). OpenMC `sample_fission_neutron` (`src/physics.cpp:1081`, `8f79d9d0`).
//!
//! # Methodology
//!
//! (SUPERSEDED 2026-11-11: the compiled-upstream differential now exists, `fission_neutron_vs_openmc_code_to_code.rs`.) Earlier note: it needed
//! `Nuclide` and `Reaction` stubs that `verification_and_validation/
//! upstream_openmc_samplers/` does not yet carry; its pieces (the uncorrelated
//! law's `mu`-then-`E` order, `rotate_angle`, the Watt/Maxwell/tabulated energy
//! laws) ARE bit-identical to upstream there. This test pins the composition by
//! REPLAY with the same LCG on U-235 (ACE, 293.6 K):
//! 1. prompt path (nuclide with delayed spectra ablated, so beta = 0 but the
//!    `prn < beta` draw is still consumed): replay `prn`, `mu = -1 + 2 prn`,
//!    `E'` from the ablated nuclide's own chi sampler, `rotate_angle(u, mu)`;
//!    pass: `E'`, direction and final RNG state bit-identical, 2000 seeds;
//! 2. delayed path (full nuclide): the fraction delayed matches `nu_d/nu_t` and
//!    the group frequencies match the evaluation's `group_fraction`, each within
//!    5 sigma; outputs are unit vectors.
//!
//! # Results (2026-11-11)
//!
//! Printed by the test.

use njoy_outram_park_fork::reference_data::ace_reference_file_or_skip;
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::scatter::rotate_angle_openmc;
use outram_mc_libs::rng::lcg::prn;

fn u235() -> Option<Nuclide> {
    let p = ace_reference_file_or_skip(
        "reference-njoy/endf-b-viii.0/293.6K/U235.ace.gz",
        "fission neutron order",
    )?;
    let raw = njoy_outram_park_fork::acer::read::read(&p).expect("read");
    Some(Nuclide::from_ace(&raw, "U235").expect("ace"))
}

#[test]
fn prompt_path_replays_openmc_draw_order() {
    let Some(full) = u235() else { return };
    let abl = full.without_delayed_spectra();
    let u = Direction::new(0.6, 0.0, 0.8);
    let mut same = 0;
    for s in 1..=2000u64 {
        let seed0 = s.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut a = seed0;
        let (g, e, d) = abl.sample_fission_neutron_openmc(1.0e6, f64::INFINITY, u, &mut a);
        assert!(g.is_none());
        let mut b = seed0;
        let _beta_draw = prn(&mut b);
        let mu = -1.0 + 2.0 * prn(&mut b);
        let e_ref = abl.sample_fission_energy_below(1.0e6, f64::INFINITY, &mut b);
        let d_ref = rotate_angle_openmc(u, mu, None, &mut b);
        if e.to_bits() == e_ref.to_bits()
            && d.u.to_bits() == d_ref.u.to_bits()
            && d.v.to_bits() == d_ref.v.to_bits()
            && d.w.to_bits() == d_ref.w.to_bits()
            && a == b
        {
            same += 1;
        }
    }
    println!("prompt replay: {same}/2000 bit-identical (E', direction, final state)");
    assert_eq!(same, 2000);
}

#[test]
fn delayed_path_fraction_and_groups_match_the_evaluation() {
    let Some(nuc) = u235() else { return };
    let u = Direction::new(0.0, 0.0, 1.0);
    let e_in = 1.0e6;
    let d = nuc.delayed().expect("delayed data");
    let n = d.n_groups();
    let beta = d.nu_delayed_at(e_in) / nuc.nu_bar(e_in);
    let total = 400_000usize;
    let mut seed = 12345u64;
    let mut counts = vec![0usize; n];
    let mut n_del = 0usize;
    for _ in 0..total {
        let (g, e, dir) = nuc.sample_fission_neutron_openmc(e_in, f64::INFINITY, u, &mut seed);
        assert!(e > 0.0);
        let norm = dir.u * dir.u + dir.v * dir.v + dir.w * dir.w;
        assert!((norm - 1.0).abs() < 1e-12);
        if let Some(k) = g {
            n_del += 1;
            counts[k] += 1;
        }
    }
    let f = n_del as f64 / total as f64;
    let sig = (beta * (1.0 - beta) / total as f64).sqrt();
    println!("delayed fraction {f:.6} vs beta {beta:.6} ({:.2} sigma)", (f - beta) / sig);
    assert!((f - beta).abs() < 5.0 * sig);
    let sum: f64 = (0..n).map(|k| d.group_fraction(k, e_in)).sum();
    for k in 0..n {
        let p = d.group_fraction(k, e_in) / sum;
        let fk = counts[k] as f64 / n_del as f64;
        let s = (p * (1.0 - p) / n_del as f64).sqrt();
        println!("group {k}: {fk:.5} vs {p:.5} ({:.2} sigma)", (fk - p) / s);
        assert!((fk - p).abs() < 5.0 * s);
    }
}
