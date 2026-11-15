// SPDX-License-Identifier: GPL-3.0

//! **Four small cross-section details, now OpenMC's** — GitHub #463 items 5, 6,
//! 7 and 11.
//!
//! Each is a place where this port used a nearby but different rule from
//! OpenMC (commit `8f79d9d`):
//!
//! | item | OpenMC | this port before |
//! |---|---|---|
//! | 5 | `nu_fission` is tabulated as `nu(E_i) sigma_f(E_i)` on the grid and interpolated (`nuclide.cpp:438-445`, `:760-761`) | `sigma_f(E) nu(E)` at the exact energy |
//! | 6 | outside the energy grid `f < 0` or `f > 1` extrapolates (`nuclide.cpp:718-744`) | clamped to the end point |
//! | 7 | S(a,b) is dropped only for `E > energy_max` (`material.cpp:866`) | dropped at `E >= cutoff` |
//! | 11 | with S(a,b): `total = total + thermal - sab_frac * elastic` (`nuclide.cpp:857`) | rebuilt as `absorption + inelastic + n2n + thermal` |
//!
//! `OUTRAM_MC_LEGACY_XS_DETAILS=1` restores the previous behaviour for all four
//! (a cached environment knob, like `OUTRAM_MC_LEGACY_SCATTER_KERNELS`), so a
//! paired run can price them. These tests pin the **new** default against
//! values computed here from the formulas above, not from the code under test.
//! They do not run under the legacy knob.
//!
//! # Results (2026-11-14)
//!
//! Printed by the tests; the priced effect on k is in the issue comment.
#![allow(deprecated)] // gh:#462 (audited 2026-11-14, gh:#1581): nuclide-level test or probe; it builds one bound nuclide and no material references it, so there is no free material that could receive the bound law. Not migrated: the nuclide-level binding is what it exercises.

use njoy_outram_park_fork::hdf5::nuclide_laws::{
    AngleDistribution, AngleEnergy, EmissionMode, EnergyDist, Function1D, Product, Tabulated1D,
};
use njoy_outram_park_fork::hdf5::nuclide_read::read_nuclide;
use njoy_outram_park_fork::hdf5::nuclide_write::{write_nuclide, NuclideData, ReactionData};
use petir::mathf::RealMath;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;

fn legacy() -> bool {
    std::env::var("OUTRAM_MC_LEGACY_XS_DETAILS").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// A fissile nuclide whose elastic, fission and total vary linearly between
/// four grid points, with a ν̄ that varies too, so every rule above gives a
/// different number from its predecessor.
fn varying_nuclide() -> (Nuclide, Vec<f64>, Vec<f64>) {
    let energy = vec![1.0e-5, 1.0e5, 2.0e6, 2.0e7];
    let fission = vec![1.0, 2.0, 3.0, 5.0];
    let elastic = vec![10.0, 9.0, 4.0, 3.0];
    let capture = vec![1.0; 4];
    let total: Vec<f64> = (0..4).map(|i| fission[i] + elastic[i] + capture[i]).collect();
    let (lo, hi) = (energy[0], energy[3]);
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
    let nu = Product {
        particle: "neutron".into(),
        emission_mode: EmissionMode::Total,
        decay_rate: 0.0,
        // nu(E) = 2 + 2 E / 2e7, linear: 2.0 at the bottom, 4.0 at the top.
        yield_: Function1D::Tabulated(Tabulated1D::lin_lin(vec![lo, hi], vec![2.0, 4.0]).unwrap()),
        distribution: vec![AngleEnergy::Uncorrelated {
            angle: Some(AngleDistribution::isotropic(vec![lo, hi])),
            energy: None,
        }],
        applicability: vec![],
    };
    let data = NuclideData {
        name: "Var".into(),
        z: 92,
        a: 233,
        metastable: 0,
        atomic_weight_ratio: 233.0248,
        temperature: "294K".into(),
        kt_ev: 2.53e-2,
        energy: energy.clone(),
        reactions: vec![
            ReactionData {
                mt: 1,
                q_value: 0.0,
                center_of_mass: false,
                xs: total.clone(),
                threshold_idx: 0,
                redundant: false,
                products: vec![],
            },
            ReactionData::elastic(elastic.clone(), AngleDistribution::isotropic(vec![lo, hi]), lo, hi)
                .unwrap(),
            ReactionData::capture(6.0e6, capture),
            ReactionData {
                mt: 18,
                q_value: 2.0e8,
                center_of_mass: false,
                xs: fission.clone(),
                threshold_idx: 0,
                redundant: false,
                products: vec![watt],
            },
        ],
        total_nu: Some(nu),
        urr: vec![],
    };
    let p = std::env::temp_dir().join("var_from_hdf5_463.h5");
    write_nuclide(&p, &data).expect("write");
    let read = read_nuclide(&p).expect("read");
    let _ = std::fs::remove_file(&p);
    (Nuclide::from_hdf5(&read, "Var").expect("from_hdf5"), energy, fission)
}

fn nu_of(e: f64) -> f64 {
    2.0 + 2.0 * (e - 1.0e-5) / (2.0e7 - 1.0e-5)
}

/// Item 5. Prediction: `nu_fission(E) = (1-f) nu(E_i) s_i + f nu(E_{i+1}) s_{i+1}`
/// at a point inside an interval, differing from `s(E) nu(E)` by
/// `f (1-f) dnu ds`.
#[test]
fn nu_fission_is_the_interpolated_product() {
    if legacy() {
        return;
    }
    let (n, energy, s) = varying_nuclide();
    let e = 1.0e6; // inside [1e5, 2e6]
    let f = (e - energy[1]) / (energy[2] - energy[1]);
    let expect = (1.0 - f) * nu_of(energy[1]) * s[1] + f * nu_of(energy[2]) * s[2];
    let exact = (s[1] + f * (s[2] - s[1])) * nu_of(e);
    let got = n.xs_at_energy(e, 293.6).nu_fission;
    println!("E = 1 MeV: interpolated product {got:.9}, formula {expect:.9}, s(E) nu(E) {exact:.9}");
    assert!((got - expect).abs() < 1e-12 * expect, "{got} vs {expect}");
    assert!((got - exact).abs() > 1e-4, "the two rules must differ here, or the test cannot fail");
    // And on a grid point the two rules coincide.
    let on = n.xs_at_energy(energy[1], 293.6).nu_fission;
    assert!((on - nu_of(energy[1]) * s[1]).abs() < 1e-12 * on);
}

/// Item 6. Prediction: below the grid `f = (E - E_0)/(E_1 - E_0) < 0` and the
/// cross section extrapolates linearly through the first two points; above it,
/// through the last two.
#[test]
fn outside_the_grid_the_cross_section_extrapolates() {
    if legacy() {
        return;
    }
    let (n, e, _) = varying_nuclide();
    // Elastic 10.0 at 1e-5 eV and 9.0 at 1e5 eV.
    let below = 0.5 * e[0];
    let f = (below - e[0]) / (e[1] - e[0]);
    let want_lo = (1.0 - f) * 10.0 + f * 9.0;
    let got_lo = n.xs_at_energy(below, 293.6).elastic;
    // Elastic 4.0 at 2e6 and 3.0 at 2e7.
    let above = 2.5e7;
    let g = (above - e[2]) / (e[3] - e[2]);
    let want_hi = (1.0 - g) * 4.0 + g * 3.0;
    let got_hi = n.xs_at_energy(above, 293.6).elastic;
    println!("elastic below grid {got_lo:.12} (want {want_lo:.12}); above {got_hi:.12} (want {want_hi:.12})");
    assert!((got_lo - want_lo).abs() < 1e-12 * want_lo);
    assert!((got_hi - want_hi).abs() < 1e-12 * want_hi.abs());
    assert!(got_lo > 10.0 && got_hi < 3.0, "extrapolation, not a clamp");
}

fn thermal() -> Option<ThermalScattering> {
    let tape = njoy_outram_park_fork::reference_data::reference_endf("tsl-HinH2O.endf")?;
    ThermalScattering::from_endf_file(tape.to_str()?, 1, 293.6, "c_H_in_H2O").ok()
}

/// Item 7. Prediction: S(a,b) is in force AT its cutoff (`E > energy_max` is
/// the drop condition) and gone above it.
#[test]
fn sab_is_in_force_at_exactly_its_cutoff() {
    if legacy() {
        return;
    }
    let Some(th) = thermal() else {
        println!("tsl-HinH2O.endf absent: skipping");
        return;
    };
    let c = th.cutoff_ev();
    let at = th.total_xs(c);
    let above = th.total_xs(c * (1.0 + 1e-9));
    println!("cutoff {c} eV: total_xs at {at:.6} b, just above {above:.6} b");
    assert!(at > 0.0, "S(a,b) must still apply at E == energy_max");
    assert_eq!(above, 0.0);
    let mut seed = 11_u64;
    assert!(th.sample(c, &mut seed).is_some());
    assert!(th.sample(c * (1.0 + 1e-9), &mut seed).is_none());
}

/// Item 11. Prediction: with S(a,b) attached, `total = total + thermal -
/// elastic` bit for bit, on both the full and the fast path.
#[test]
fn the_sab_total_is_upstreams_rule() {
    if legacy() {
        return;
    }
    let Some(tape) = njoy_outram_park_fork::reference_data::reference_endf("n-001_H_001-ENDF8.0-Beta6.endf")
    else {
        println!("H-1 tape absent: skipping");
        return;
    };
    let Some(th) = thermal() else {
        println!("tsl-HinH2O.endf absent: skipping");
        return;
    };
    let t = uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(293.6);
    let free = Nuclide::from_endf_file(&tape, "H1", t, 1.0e-3).expect("H1");
    let bound = Nuclide::from_endf_file(&tape, "H1", t, 1.0e-3)
        .expect("H1")
        .with_thermal_scattering(th.clone());
    let mut worst = 0.0f64;
    for k in 0..200 {
        let e = 1.0e-4 * (th.cutoff_ev() / 1.0e-4).r_powf(k as f64 / 199.0);
        let b = free.xs_at_energy(e, 293.6);
        let sab = th.total_xs(e);
        let want = b.total + sab - b.elastic;
        let x = bound.xs_at_energy(e, 293.6);
        let fast = bound.total_at_energy(e, 293.6);
        assert_eq!(x.total.to_bits(), want.to_bits(), "full path at {e}");
        assert_eq!(fast.to_bits(), want.to_bits(), "fast path at {e}");
        // How far this moved from the old rebuild.
        let old = x.absorption + x.inelastic + x.n2n + sab;
        worst = worst.max(((want - old) / want).abs());
    }
    println!("largest relative move of the S(a,b) total against the old rebuild: {worst:.3e}");
}
