// SPDX-License-Identifier: GPL-3.0

//! **Sampling a delayed-neutron precursor group** — gh:#262 scope items 2–3.
//!
//! # What this is and is not
//!
//! This verifies `Nuclide::sample_delayed_group` **on its own**: that the
//! delayed/prompt split reproduces the evaluation's own `beta(E) = nu_d/nu`, and
//! that the group draw reproduces the evaluation's own normalised `p_k(E)`.
//!
//! It does **not** make `DelayedGroupFilter` work, and it does not make IFP's
//! `beta_eff` work. Nothing in the transport loop calls this yet: `Site` carries
//! no precursor group, so a tag cannot travel with a banked fission neutron
//! across generations, which is what IFP needs. That plumbing is a separate
//! change — 59 `Site {…}` literals across the crate — and doing it at speed on
//! the fission path is how a plausible wrong `k` gets produced.
//!
//! Saying so matters because "the pieces exist and nothing joins them" is the
//! exact defect shape this session has found four times (#258's split daughters,
//! #264's surface source, #266's integrators, #262's IFP). This is deliberately
//! a sub-step, labelled as one.
//!
//! # Method
//!
//! The reference is **the evaluation itself**, not a stored number: every
//! expected value is read from `DelayedData` at the same energy the sampler is
//! asked about, so the test cannot drift from the data and cannot be satisfied
//! by a tuned constant.
//!
//! # Results (2026-09-24, ENDF/B-VIII.0 U-235 and U-238)
//!
//! Printed by the tests.

use outram_mc_libs::material::nuclide::Nuclide;

const TEMP: f64 = 293.6;
const TOL: f64 = 5.0e-3;
const N_DRAWS: usize = 400_000;

fn nuclide(file: &str, name: &str) -> Option<Nuclide> {
    let p = njoy_outram_park_fork::reference_data::reference_endf_or_skip(
        file,
        &format!("{name} (delayed-group sampling)"),
    )?;
    Some(Nuclide::from_endf_file(&p, name, TEMP, 5.0e-3).expect("reconstructs"))
}

/// **The delayed fraction the sampler realises is the evaluation's own
/// `beta(E)`.** Not a stored number — read from the same data at the same
/// energy.
#[test]
fn the_delayed_fraction_reproduces_the_evaluations_beta() {
    for (file, name) in [
        ("n-092_U_235.endf", "U235"),
        ("n-092_U_238.endf", "U238"),
    ] {
        let Some(nuc) = nuclide(file, name) else {
            continue;
        };
        let Some(d) = nuc.delayed() else {
            println!("[skip] {name} carries no MF=1/455");
            continue;
        };
        println!("{name}: {} precursor groups", d.n_groups());
        for &e in &[0.0253_f64, 1.0e3, 1.0e6, 1.4e7] {
            let beta = nuc.delayed_fraction(e);
            if beta <= 0.0 {
                continue;
            }
            let mut seed = 0x5eed_0000_0000_0001_u64 ^ (e.to_bits());
            let mut delayed = 0usize;
            for _ in 0..N_DRAWS {
                if nuc.sample_delayed_group(e, &mut seed).is_some() {
                    delayed += 1;
                }
            }
            let realised = delayed as f64 / N_DRAWS as f64;
            // Binomial 1-sigma on the realised fraction.
            let sigma = (beta * (1.0 - beta) / N_DRAWS as f64).sqrt();
            let z = (realised - beta).abs() / sigma.max(f64::MIN_POSITIVE);
            println!(
                "  E={e:9.4e} eV  beta={beta:.6}  realised={realised:.6}  \
                 {z:.2} sigma"
            );
            assert!(
                z < 5.0,
                "{name} at {e:.3e} eV: sampled delayed fraction {realised:.6} against \
                 the evaluation's beta {beta:.6} is {z:.1} sigma of the binomial \
                 {sigma:.2e}. The split must reproduce nu_d/nu or every beta_eff \
                 built on it is scaled wrong."
            );
        }
    }
}

/// **The group distribution reproduces the evaluation's normalised `p_k(E)`.**
///
/// The normalisation is the point: `DelayedData::group_fraction` interpolates
/// each group's table independently, so the shares at an arbitrary `E` need not
/// sum to 1. Comparing against the raw tables would fail for a correct sampler;
/// comparing against them normalised is the only statement that holds.
#[test]
fn the_group_shares_reproduce_the_normalised_tables() {
    for (file, name) in [
        ("n-092_U_235.endf", "U235"),
        ("n-092_U_238.endf", "U238"),
    ] {
        let Some(nuc) = nuclide(file, name) else {
            continue;
        };
        let Some(d) = nuc.delayed() else {
            continue;
        };
        let n = d.n_groups();
        assert!(n > 0, "{name} reports zero precursor groups");
        for &e in &[0.0253_f64, 1.0e6] {
            if nuc.delayed_fraction(e) <= 0.0 {
                continue;
            }
            let raw: Vec<f64> = (0..n).map(|k| d.group_fraction(k, e).max(0.0)).collect();
            let sum: f64 = raw.iter().sum();
            if sum <= 0.0 {
                continue;
            }
            let want: Vec<f64> = raw.iter().map(|s| s / sum).collect();

            let mut seed = 0xabcd_0000_0000_0003_u64 ^ e.to_bits();
            let mut counts = vec![0usize; n];
            let mut total = 0usize;
            // Draw until enough DELAYED neutrons have been produced; the
            // prompt ones carry no group.
            while total < N_DRAWS {
                if let Some(k) = nuc.sample_delayed_group(e, &mut seed) {
                    counts[k] += 1;
                    total += 1;
                }
            }
            println!("{name} at {e:.4e} eV, raw shares sum to {sum:.6}:");
            for k in 0..n {
                let realised = counts[k] as f64 / total as f64;
                let sigma = (want[k] * (1.0 - want[k]) / total as f64).sqrt();
                let z = (realised - want[k]).abs() / sigma.max(f64::MIN_POSITIVE);
                println!(
                    "   group {k}: want {:.6}  realised {realised:.6}  {z:.2} sigma",
                    want[k]
                );
                assert!(
                    z < 5.0 || (realised - want[k]).abs() < TOL,
                    "{name} group {k} at {e:.3e} eV: {realised:.6} against the \
                     normalised table's {:.6}, {z:.1} sigma",
                    want[k]
                );
            }
            let realised_sum: f64 = counts.iter().map(|&c| c as f64).sum::<f64>() / total as f64;
            assert!(
                (realised_sum - 1.0).abs() < 1.0e-12,
                "the sampled group shares must sum to 1, got {realised_sum}"
            );
        }
    }
}

/// A nuclide with **no** MF=1/455 reports every neutron prompt, rather than
/// inventing a group. `None` there means *"this evaluation does not say"*, which
/// is a different statement from a zero delayed fraction.
#[test]
fn a_nuclide_without_delayed_data_reports_every_neutron_prompt() {
    let Some(nuc) = nuclide("n-008_O_016.endf", "O16") else {
        return;
    };
    let mut seed = 7u64;
    for _ in 0..1000 {
        assert!(
            nuc.sample_delayed_group(1.0e6, &mut seed).is_none(),
            "O-16 has no delayed-neutron data, so no group can be sampled for it"
        );
    }
    println!(
        "O16: delayed() = {}, every draw prompt",
        nuc.delayed().is_some()
    );
}
