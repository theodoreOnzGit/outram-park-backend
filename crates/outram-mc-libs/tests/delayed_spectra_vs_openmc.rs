// SPDX-License-Identifier: GPL-3.0

//! **Delayed fission neutrons are born with their own spectra, on both data
//! routes** — GitHub #365 audit.
//!
//! # What was wrong
//!
//! Every fission neutron, delayed ones included, was drawn from the prompt χ.
//! The ACE route never read DNEDL/DNED. The ENDF route parsed MF=5/455 but kept
//! the spectrum only as pairs that were not samplable: the `g(x)` of U-235 and
//! U-238 is a *histogram*. OpenMC's `sample_fission_neutron`
//! (`src/physics.cpp`) draws a delayed neutron with probability `nu_d/nu_t`,
//! its group by yield, and its energy from that group's law. Delayed spectra
//! average ~0.51 MeV against ~2 MeV prompt.
//!
//! # Methodology
//!
//! Reference: the exact mean birth energy under that scheme,
//! `(1 - beta) <E>_prompt + beta sum_k w_k <E>_k`. It is computed by
//! `verification_and_validation/ace_route_physics/openmc_inputs/delayed_spectra_reference.py`
//! from OpenMC's readers (0.16.1.dev25) into `data/delayed_spectra_openmc.csv`:
//! - ACE rows: OpenMC's ACE reader of the NJOY2016 tables;
//! - ENDF rows: OpenMC's ENDF reader of the evaluations, which is independent
//!   of NJOY and of this crate's MF=5/455 parser.
//!
//! For U-235 and U-238, both routes, at 1 and 5 MeV, with `N = 2 000 000`
//! draws of `Nuclide::sample_fission_energy`:
//! - the mean must lie within **5 sigma** (sample sem) of the mixture
//!   reference;
//! - the ablated arm (`without_delayed_spectra`, prompt χ for every neutron)
//!   must lie within 5 sigma of the **prompt-only** reference `<E>_prompt`;
//! - the separation `|<E>_prompt - mixture| / sem` must exceed 5, so that the
//!   two references are resolvable at this N and the first check can fail.
//!
//! The separation is beta (<E>_p - <E>_d): 7.5 keV (U-235, 5 MeV) to 25 keV
//! (U-238, 1 MeV) against a sem of about 1.1 keV.
//!
//! **A correction to this test's first version.** It required the ablated arm
//! to miss the mixture by more than 5 sigma of one run. At U-235 5 MeV, where
//! the expected miss is about 6.8 sigma, a draw gave 3.9 sigma. That was a
//! power criterion subject to its own statistics, not a physics failure. The
//! checks above compare each arm with its own exact reference instead, and put
//! the power requirement on the references, where it has no noise.
//!
//! ACE tables: `reference-data/ace` (NJOY2016). ENDF: `reference-data/endf`,
//! built at `SpeedTier::VeryFast`, because RECONR's tolerance does not touch
//! chi, nu or the delayed data this test reads.
//!
//! # Results (2026-09-29)
//!
//! Printed by the test; recorded in the #365 thread.

use njoy_outram_park_fork::reference_data::{ace_reference_file_or_skip, reference_endf};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::speed::SpeedTier;
use std::path::PathBuf;

const N: usize = 2_000_000;

fn mean_sem(n: &Nuclide, e: f64, seed: u64) -> (f64, f64) {
    let mut s = seed;
    let v: Vec<f64> = (0..N).map(|_| n.sample_fission_energy(e, &mut s)).collect();
    let m = v.iter().sum::<f64>() / N as f64;
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (N - 1) as f64;
    (m, (var / N as f64).sqrt())
}

#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "reconstructs U-235 and U-238 from ENDF; runs by default"
)]
fn delayed_spectra_match_openmc_on_both_routes() {
    let csv = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/ace_route_physics/data/delayed_spectra_openmc.csv");
    let text = std::fs::read_to_string(csv).expect("reference");
    let tapes = [
        ("U235", "n-092_U_235-ENDF8.0.endf"),
        ("U238", "n-092_U_238.endf"),
    ];
    let mut nucs: Vec<(String, String, Nuclide)> = Vec::new();
    for (name, tape) in tapes {
        let Some(p) = ace_reference_file_or_skip(
            &format!("reference-njoy/endf-b-viii.0/293.6K/{name}.ace.gz"),
            "delayed spectra",
        ) else {
            return;
        };
        let raw = njoy_outram_park_fork::acer::read::read(&p).expect("read");
        nucs.push((
            "ace".into(),
            name.into(),
            Nuclide::from_ace(&raw, name).expect("ace"),
        ));
        let t = reference_endf(tape).expect("tape");
        nucs.push((
            "endf".into(),
            name.into(),
            Nuclide::from_endf_file_with_speed(&t, name, 293.6, SpeedTier::VeryFast).expect("endf"),
        ));
    }
    let mut rows = 0;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split(',').collect();
        let (route, name) = (f[0], f[1]);
        let e: f64 = f[2].parse().unwrap();
        let mix: f64 = f[6].parse().unwrap();
        let (_, _, nuc) = nucs
            .iter()
            .find(|(r, n, _)| r == route && n == name)
            .expect("nuclide");
        assert!(
            nuc.applies_delayed_spectra(),
            "{route} {name}: delayed spectra must be on by default"
        );
        let (m, s) = mean_sem(nuc, e, 0xDE1A_0000 + rows as u64);
        let ablated = nuc.clone().without_delayed_spectra();
        assert!(!ablated.applies_delayed_spectra());
        let (ma, sa) = mean_sem(&ablated, e, 0xDE1A_1000 + rows as u64);
        let prompt: f64 = f[4].parse().unwrap();
        let z = (m - mix) / s;
        let za = (ma - prompt) / sa;
        let sep = (prompt - mix).abs() / s;
        println!(
            "{route} {name} E = {:.0} MeV: <E'> = {:.3} keV vs mixture {:.3} (z {z:+.2}); \
             ablated {:.3} vs prompt {:.3} (z {za:+.2}); separation {sep:.1} sigma",
            e / 1e6,
            m / 1e3,
            mix / 1e3,
            ma / 1e3,
            prompt / 1e3
        );
        assert!(z.abs() < 5.0, "{route} {name} E = {e}: z = {z}");
        assert!(
            za.abs() < 5.0,
            "{route} {name} E = {e}: ablated arm vs prompt z = {za}"
        );
        assert!(
            sep > 5.0,
            "{route} {name} E = {e}: references not resolvable ({sep:.1} sigma)"
        );
        rows += 1;
    }
    assert_eq!(rows, 8);
}
