// SPDX-License-Identifier: GPL-3.0

//! **How big is OpenMC's `nu_fission` interpolation on a real evaluation?** —
//! GitHub #463 item 5.
//!
//! OpenMC interpolates the tabulated product `nu(E_i) sigma_f(E_i)`; this
//! port multiplied `sigma_f(E) nu(E)` at the exact energy. They agree on a grid
//! point and differ inside an interval by `f (1-f) dnu dsigma_f`. This test
//! measures that difference on U-235 (ENDF/B-VIII.0, RECONR + BROADR at
//! 293.6 K, tolerance 1e-3) over 20 000 log-spaced energies from 1e-5 eV to
//! 20 MeV, and over the fast range 100 keV to 10 MeV where Godiva lives.
//!
//! Prediction, written before the run: nu is smooth (a few per cent over a
//! decade above 1 MeV) and RECONR's 1e-3 tolerance keeps `sigma_f` close to
//! linear inside each interval, so the relative difference is far below 1e-3
//! in the fast range, and its mean is zero to first order (the product of a
//! convex and a concave piece changes sign). Larger values are allowed only
//! inside resonances, which are a thermal-range effect.
//!
//! # Results (2026-11-14)
//!
//! Printed by the test.

#![cfg(feature = "std")] // needs files, threads or reports (gh:#1605)

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::nuclide::Nuclide;
use petir::mathf::RealMath;

#[test]
fn the_interpolated_product_differs_from_the_exact_product_by_a_small_amount() {
    if std::env::var("OUTRAM_MC_LEGACY_XS_DETAILS").is_ok_and(|v| !v.is_empty() && v != "0") {
        return;
    }
    let Some(p) = reference_endf("n-092_U_235-ENDF8.0.endf") else {
        println!("U-235 tape absent: skipping");
        return;
    };
    let t = uom::si::f64::ThermodynamicTemperature::new::<
        uom::si::thermodynamic_temperature::kelvin,
    >(293.6);
    let n = Nuclide::from_endf_file(&p, "U235", t, 1.0e-3).expect("U-235");
    let (lo, hi) = (1.0e-5_f64, 2.0e7_f64);
    let m = 20_000;
    let (mut worst_all, mut worst_fast) = (0.0f64, 0.0f64);
    let (mut sum_fast, mut n_fast) = (0.0f64, 0usize);
    for k in 0..m {
        let e = lo * (hi / lo).r_powf(f64::from(k) / f64::from(m - 1));
        let x = n.xs_at_energy(e, 293.6);
        if x.fission <= 0.0 {
            continue;
        }
        let rel = (x.nu_fission - x.fission * n.nu_bar(e)) / x.nu_fission;
        worst_all = worst_all.max(rel.abs());
        if (1.0e5..=1.0e7).contains(&e) {
            worst_fast = worst_fast.max(rel.abs());
            sum_fast += rel;
            n_fast += 1;
        }
    }
    println!(
        "U-235 nu_fission, interpolated product vs exact product: worst |rel| {worst_all:.3e} \
         over 1e-5 eV..20 MeV; worst {worst_fast:.3e}, mean {:+.3e} over 0.1..10 MeV ({n_fast} points)",
        sum_fast / n_fast as f64
    );
    assert!(worst_fast < 1.0e-3, "fast-range difference {worst_fast:.3e}");
}
