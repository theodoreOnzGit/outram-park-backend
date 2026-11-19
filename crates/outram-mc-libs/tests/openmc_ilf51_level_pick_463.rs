// SPDX-License-Identifier: GPL-3.0

//! **The inelastic level pick inside the unresolved range with `ILF = 51`** —
//! GitHub #463 item 3.
//!
//! # What OpenMC does
//!
//! In the URR with a competing inelastic reaction, `calculate_urr_xs` sets the
//! inelastic cross section to that one reaction's smooth value
//! (`nuclide.cpp:946-958`), so the nuclide's scattering block is
//! `elastic + sigma_ILF`. `scatter()` draws `cutoff = prn * (total - absorption)`
//! and, past the elastic part, walks the inelastic reactions **in order**,
//! adding each one's cross section until `prob >= cutoff`
//! (`physics.cpp:719-763`). With `ILF = 51` the cutoff measured from the start
//! of the inelastic block is at most `sigma_51`, and the first reaction in the
//! walk is MT=51, so MT=51 is always chosen: MT=52 is never reached inside the
//! window (U-238, 148.4-149 keV).
//!
//! # What this port does
//!
//! The default (OpenMC) collision kernel passes that same cutoff to
//! `Nuclide::sample_inelastic_at`, which walks the levels in the same order, so
//! the pick is already OpenMC's. What picked by smooth cross section was the
//! legacy ladder's fresh draw `prn * sum(sigma)` (`Inelastic(None)`).
//!
//! # Method
//!
//! A two-level nuclide written by the crate's `.h5` writer: MT=51 (1.0 b above
//! 1 MeV) and MT=52 (0.5 b above 2 MeV), read back with `Nuclide::from_hdf5`.
//! Prediction, written before the run: every cutoff in `[0, sigma_51]` returns
//! MT=51; a cutoff just above it returns MT=52; the legacy fresh draw returns
//! MT=52 at `sigma_52 / (sigma_51 + sigma_52)` of draws.
//!
//! # Results (2026-11-14)
//!
//! Printed by the test.

#![cfg(feature = "std")] // needs files, threads or reports (gh:#1605)

use njoy_outram_park_fork::hdf5::nuclide_laws::{
    AngleDistribution, AngleEnergy, EnergyDist, Product,
};
use njoy_outram_park_fork::hdf5::nuclide_read::read_nuclide;
use njoy_outram_park_fork::hdf5::nuclide_write::{write_nuclide, NuclideData, ReactionData};
use outram_mc_libs::material::nuclide::{Inelastic, Nuclide};
use petir::mathf::RealMath;

fn two_level_nuclide() -> Nuclide {
    let awr = 235.0;
    let thr = |q: f64| (awr + 1.0) / awr * (-q);
    let (t51, t52) = (thr(-1.0e6), thr(-2.0e6));
    let mut energy = vec![1.0e-5, 5.0e5, 1.0e7, 2.0e7];
    energy.extend([t51, t52]);
    energy.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (lo, hi) = (energy[0], *energy.last().unwrap());
    let n = energy.len();
    let level = |q: f64, t: f64, sigma: f64| {
        let ti = energy.iter().position(|&e| e == t).unwrap();
        let xs: Vec<f64> = (ti..n).map(|i| if i == ti { 0.0 } else { sigma }).collect();
        let product = Product::prompt_neutron(
            AngleEnergy::Uncorrelated {
                angle: Some(AngleDistribution::isotropic(vec![t, hi])),
                energy: Some(EnergyDist::Level {
                    threshold: t,
                    mass_ratio: (awr / (awr + 1.0)).r_powi(2),
                }),
            },
            t,
            hi,
        )
        .unwrap();
        (q, ti, xs, product)
    };
    let (q51, i51, x51, p51) = level(-1.0e6, t51, 1.0);
    let (q52, i52, x52, p52) = level(-2.0e6, t52, 0.5);
    let total: Vec<f64> = (0..n)
        .map(|i| {
            5.0 + 1.0
                + if i >= i51 { x51[i - i51] } else { 0.0 }
                + if i >= i52 { x52[i - i52] } else { 0.0 }
        })
        .collect();
    let data = NuclideData {
        name: "Lev2".into(),
        z: 92,
        a: 235,
        metastable: 0,
        atomic_weight_ratio: awr,
        temperature: "294K".into(),
        kt_ev: 2.53e-2,
        energy: energy.clone(),
        reactions: vec![
            ReactionData {
                mt: 1,
                q_value: 0.0,
                center_of_mass: false,
                xs: total,
                threshold_idx: 0,
                redundant: false,
                products: vec![],
            },
            ReactionData::elastic(
                vec![5.0; n],
                AngleDistribution::isotropic(vec![lo, hi]),
                lo,
                hi,
            )
            .unwrap(),
            ReactionData::capture(6.0e6, vec![1.0; n]),
            ReactionData {
                mt: 51,
                q_value: q51,
                center_of_mass: true,
                xs: x51,
                threshold_idx: i51 as i64,
                redundant: false,
                products: vec![p51],
            },
            ReactionData {
                mt: 52,
                q_value: q52,
                center_of_mass: true,
                xs: x52,
                threshold_idx: i52 as i64,
                redundant: false,
                products: vec![p52],
            },
        ],
        total_nu: None,
        urr: vec![],
    };
    let p = std::env::temp_dir().join("lev2_from_hdf5_463.h5");
    write_nuclide(&p, &data).expect("write");
    let read = read_nuclide(&p).expect("read");
    let _ = std::fs::remove_file(&p);
    Nuclide::from_hdf5(&read, "Lev2").expect("from_hdf5")
}

#[test]
fn a_cutoff_inside_the_ilf51_block_always_picks_mt51() {
    let n = two_level_nuclide();
    let e = 5.0e6;
    // Cross sections rise linearly from zero at each threshold to the next
    // grid point, so read the values rather than assuming the plateaus.
    let s51 = n.reaction_xs(51, e).unwrap();
    let s52 = n.reaction_xs(52, e).unwrap();
    let x = n.xs_at_energy(e, 293.6);
    println!("E = 5 MeV: sigma_51 {s51:.6} b, sigma_52 {s52:.6} b, inelastic {:.6} b", x.inelastic);
    assert!(s51 > 0.0 && s52 > 0.0, "both levels must be open");
    assert!((x.inelastic - (s51 + s52)).abs() < 1e-12);
    let mut seed = 5_u64;
    for k in 0..=100 {
        let c = s51 * f64::from(k) / 100.0; // 0 ..= sigma_51
        match n.sample_inelastic_at(e, Some(c), &mut seed) {
            Inelastic::Level { mt, .. } => assert_eq!(mt, 51, "cutoff {c}"),
            other => panic!("{other:?}"),
        }
    }
    let above = n.sample_inelastic_at(e, Some(s51 + 0.5 * s52), &mut seed);
    assert!(matches!(above, Inelastic::Level { mt: 52, .. }), "{above:?}");
    let draws = 6000;
    let legacy_mt52 = (0..draws)
        .filter(|_| matches!(n.sample_inelastic(e, &mut seed), Inelastic::Level { mt: 52, .. }))
        .count();
    let expected = s52 / (s51 + s52);
    println!(
        "legacy fresh draw picked MT=52 in {legacy_mt52}/{draws} (expected {:.0})",
        expected * f64::from(draws)
    );
    assert!(
        (legacy_mt52 as f64 / f64::from(draws) - expected).abs() < 0.03,
        "the legacy draw must reach MT=52 at sigma_52/sigma_inel, or the contrast is empty"
    );
}
