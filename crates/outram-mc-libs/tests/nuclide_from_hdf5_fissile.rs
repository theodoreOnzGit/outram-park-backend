// SPDX-License-Identifier: GPL-3.0

//! **A fissile nuclide written by this workspace's own `.h5` writer, converted
//! and transported** — GitHub #303.
//!
//! The nuclide `Syn` is invented (flat elastic 5 b, capture 1 b, fission 2 b,
//! a 0.5 b `level` channel opening at 1 MeV, nu = 2.5, a Watt spectrum with
//! `a = 0.988 MeV`, `b = 2.249 /MeV`, `U = -20 MeV`). It exists because a real
//! U-235 cannot be written (the writer refuses the rank-2 attributes the
//! continuum laws need) and cannot be converted (those laws are not decoded);
//! `Syn` uses only laws both halves handle, so the whole chain
//! write -> read -> `Nuclide::from_hdf5` -> `run_keff` runs.
//!
//! # Predictions, written before the run
//!
//! - thermal `nu*sigma_f = 2.5 * 2 = 5.0 b` exactly, total 8.0 b;
//! - mean fission energy `3a/2 + a^2 b/4 = 2.031 MeV`, within 3 sigma of 20 000
//!   draws (the restriction `E' <= E - U` does not bite at `U = -20 MeV`);
//! - a fixed-seed short bare-sphere run reproduces `k` bit for bit.
//!
//! # What this does not show
//!
//! It does not compare against another code, and `k` of this invented nuclide
//! is not a benchmark. It shows the converted nuclide's channels and transport
//! plumbing work; the cross-section parity against the ACE route is
//! `nuclide_from_hdf5.rs`.

#![cfg(feature = "std")] // needs files, threads or reports (gh:#1605)

use njoy_outram_park_fork::hdf5::nuclide_laws::{
    AngleDistribution, AngleEnergy, EmissionMode, EnergyDist, Function1D, Product, Tabulated1D,
};
use njoy_outram_park_fork::hdf5::nuclide_read::{read_nuclide, ReadDistribution};
use njoy_outram_park_fork::hdf5::nuclide_write::{write_nuclide, NuclideData, ReactionData};
use outram_mc_libs::material::material::{Material, NuclideComponent};
use petir::mathf::RealMath;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{run_keff, KeffSettings};

fn synthetic_fissile_file(tag: &str) -> std::path::PathBuf {
    let n = 60usize;
    let mut energy: Vec<f64> = (0..n)
        .map(|i| 1.0e-5 * (2.0e7_f64 / 1.0e-5).r_powf(i as f64 / (n - 1) as f64))
        .collect();
    let awr = 233.0248;
    let q = -1.0e6;
    let thr = (awr + 1.0) / awr * (-q);
    energy.push(thr);
    energy.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let ti = energy.iter().position(|&e| e == thr).unwrap();
    let (lo, hi) = (energy[0], *energy.last().unwrap());
    let m = energy.len();
    let level_xs: Vec<f64> = (ti..m).map(|i| if i == ti { 0.0 } else { 0.5 }).collect();
    let watt = Product::prompt_neutron(
        AngleEnergy::Uncorrelated {
            angle: None,
            energy: Some(EnergyDist::Watt {
                u: -2.0e7,
                a: Tabulated1D::constant(0.988e6, lo, hi).unwrap(),
                b: Tabulated1D::constant(2.249e-6, lo, hi).unwrap(),
            }),
        },
        lo,
        hi,
    )
    .unwrap();
    let level = Product::prompt_neutron(
        AngleEnergy::Uncorrelated {
            angle: Some(AngleDistribution::isotropic(vec![thr, hi])),
            energy: Some(EnergyDist::Level {
                threshold: thr,
                mass_ratio: (awr / (awr + 1.0)).r_powi(2),
            }),
        },
        thr,
        hi,
    )
    .unwrap();
    let total: Vec<f64> = (0..m)
        .map(|i| 5.0 + 1.0 + 2.0 + if i >= ti { level_xs[i - ti] } else { 0.0 })
        .collect();
    let nu = Product {
        particle: "neutron".into(),
        emission_mode: EmissionMode::Total,
        decay_rate: 0.0,
        yield_: Function1D::Tabulated(Tabulated1D::constant(2.5, lo, hi).unwrap()),
        distribution: vec![AngleEnergy::Uncorrelated {
            angle: Some(AngleDistribution::isotropic(vec![lo, hi])),
            energy: None,
        }],
        applicability: vec![],
    };
    let data = NuclideData {
        name: "Syn".into(),
        z: 92,
        a: 233,
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
                vec![5.0; m],
                AngleDistribution::isotropic(vec![lo, hi]),
                lo,
                hi,
            )
            .unwrap(),
            ReactionData {
                mt: 51,
                q_value: q,
                center_of_mass: true,
                xs: level_xs,
                threshold_idx: ti as i64,
                redundant: false,
                products: vec![level],
            },
            ReactionData::capture(6.0e6, vec![1.0; m]),
            ReactionData {
                mt: 18,
                q_value: 2.0e8,
                center_of_mass: false,
                xs: vec![2.0; m],
                threshold_idx: 0,
                redundant: false,
                products: vec![watt],
            },
        ],
        total_nu: Some(nu),
        urr: vec![],
    };
    let p = std::env::temp_dir().join(format!("syn_fissile_from_hdf5_{tag}.h5"));
    write_nuclide(&p, &data).expect("write synthetic fissile nuclide");
    p
}

/// Cross sections, nu, the Watt spectrum, and a fixed-seed run.
///
/// # Results (2026-11-14)
///
/// Printed by the test.
#[test]
fn a_fissile_nuclide_from_the_writer_converts_and_transports() {
    let p = synthetic_fissile_file("a");
    let read = read_nuclide(&p).expect("read");
    let _ = std::fs::remove_file(&p);
    let n = Nuclide::from_hdf5(&read, "Syn").expect("from_hdf5");

    let x = n.xs_at_energy(2.53e-2, 293.6);
    assert_eq!((x.elastic, x.fission), (5.0, 2.0));
    assert!((x.total - 8.0).abs() < 1e-12, "total {}", x.total);
    assert!((x.nu_fission - 5.0).abs() < 1e-12, "nu_fission {}", x.nu_fission);
    let y = n.xs_at_energy(2.0e6, 293.6);
    assert!((y.inelastic - 0.5).abs() < 1e-12, "inelastic {}", y.inelastic);

    let mut seed = 12_345_u64;
    let m = 20_000usize;
    let (mut s, mut s2) = (0.0, 0.0);
    for _ in 0..m {
        let e = n.sample_fission_energy(2.53e-2, &mut seed);
        s += e;
        s2 += e * e;
    }
    let mean = s / m as f64;
    let sem = ((s2 / m as f64 - mean * mean) / m as f64).sqrt();
    let expect = 1.5 * 0.988e6 + 0.988e6 * 0.988e6 * 2.249e-6 / 4.0;
    println!("Watt mean {mean:.0} +- {sem:.0} eV, expected {expect:.0}");
    assert!((mean - expect).abs() < 3.0 * sem, "mean {mean} vs {expect}");

    let material = Material {
        id: 1,
        name: "syn".into(),
        temperature: 293.6,
        components: vec![NuclideComponent {
            nuclide_idx: 0,
            atom_density: 0.04,
        }],
    };
    let run = |seed: u64| {
        let settings = KeffSettings {
            n_particles: 400,
            n_inactive: 5,
            n_active: 10,
            temperature_k: 293.6,
            seed,
            ..KeffSettings::default()
        };
        run_keff(12.0, &material, std::slice::from_ref(&n), &settings).k_mean
    };
    let (k1, k2) = (run(7), run(7));
    println!("short bare-sphere run, seed 7: k = {k1:.5}");
    assert!(k1.is_finite() && k1 > 0.3 && k1 < 3.0, "k = {k1}");
    assert_eq!(k1.to_bits(), k2.to_bits(), "a fixed seed must reproduce k exactly");
}

/// A law the converter does not decode is refused by name.
#[test]
fn an_undecoded_law_is_refused_by_name() {
    let p = synthetic_fissile_file("b");
    let mut read = read_nuclide(&p).expect("read");
    let _ = std::fs::remove_file(&p);
    read.reactions.get_mut(&18).unwrap().products[0].distributions =
        vec![ReadDistribution::Undecoded("correlated".into())];
    let err = match Nuclide::from_hdf5(&read, "Syn") {
        Ok(_) => panic!("must refuse"),
        Err(e) => e.to_string(),
    };
    println!("{err}");
    assert!(err.contains("correlated") && err.contains("MT=18"), "{err}");
}

/// URR tables in the file are refused, not silently dropped.
#[test]
fn urr_tables_are_refused() {
    let p = synthetic_fissile_file("c");
    let mut read = read_nuclide(&p).expect("read");
    let _ = std::fs::remove_file(&p);
    read.urr_temperatures = vec!["294K".into()];
    let err = match Nuclide::from_hdf5(&read, "Syn") {
        Ok(_) => panic!("must refuse"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("unresolved-resonance"), "{err}");
}
