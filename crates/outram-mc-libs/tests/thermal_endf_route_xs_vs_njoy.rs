// SPDX-License-Identifier: GPL-3.0

//! **The ENDF-route S(α,β) inelastic cross section is THERMR's own `calcem`
//! integral, as NJOY writes it into the table OpenMC reads.** GitHub #188, and
//! the OpenMC-vs-outram-mc audit (#407).
//!
//! # What was wrong
//!
//! `ThermalScattering::from_tape` took σ_inel(E) from
//! `IncoherentInelasticScattering::inelastic_xs`. That is an independent
//! analytic E'-integral of the kernel, and it is jagged at the ±1 % level
//! between neighbouring energies. NJOY's ITIX, which OpenMC reads, is THERMR's
//! `calcem` `xsi`: the trapezoidal integral of the same rows the emission
//! tables come from (`thermr.f90:2166-2173`), put on the PENDF grid by `terp`.
//! This workspace's ACE writer already switched to `xsi` on 2026-09-29, when
//! the analytic integral was found up to 6.6 % off NJOY's ITIX. The route-4
//! transport table had not followed.
//!
//! # Methodology
//!
//! The reference is NJOY2016's H-in-H2O ACE table from the five-route library
//! (`target/five_route_keff/njoy/293.6K/HinH2O.ace`; the upstream OpenMC deck,
//! 293.6 K). This test compares its inelastic cross section, at every table
//! energy below the cutoff, against
//! `ThermalScattering::from_endf_file(tsl-HinH2O, 293.6 K).inelastic_xs`.
//!
//! Pass: the worst relative difference is ≤ 0.5 %. That bound was fixed
//! before the fix: it is the size of lin-lin interpolation on a log grid, well
//! below the analytic integral's jaggedness.
//!
//! # Results (2026-09-30)
//!
//! Over 284 energies from 1e-5 to 10 eV:
//!
//! | build | worst difference |
//! |---|---|
//! | before (analytic integral, tol 0.05) | −6.595 % at 1.0e-5 eV |
//! | xsi at tol 0.05 | −1.832 % |
//! | **xsi at OpenMC's tol 0.001, on THERMR's own grid (the fix)** | **−0.015 % at 0.416 eV** |
//!
//! The same fix takes H(H2O) against THERMR's MT=222 from −1.39 % to +0.02 %,
//! and graphite's MT=229 to +0.06 % (`tests/thermal_laws_vs_njoy_thermr.rs`).

use outram_mc_libs::material::thermal::ThermalScattering;
use std::path::PathBuf;

#[test]
fn endf_route_inelastic_xs_matches_njoy_itix() {
    let ace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/five_route_keff/njoy/293.6K/HinH2O.ace");
    let Some(tape) = njoy_outram_park_fork::reference_data::reference_endf("tsl-HinH2O.endf") else {
        println!("tsl-HinH2O.endf absent: skipping");
        return;
    };
    if !ace.is_file() {
        println!("{} absent: skipping", ace.display());
        return;
    }
    let raw = njoy_outram_park_fork::acer::read::read(&ace).expect("read");
    let njoy = ThermalScattering::from_ace(&raw, "H in H2O").expect("ACE loads");
    let ours = ThermalScattering::from_endf_file(tape.to_str().unwrap(), 1, 293.6, "H in H2O")
        .expect("builds");
    let (mut worst, mut worst_e, mut n) = (0.0_f64, 0.0_f64, 0);
    let mut e = 1.0e-5_f64;
    while e < 0.999 * ours.cutoff_ev().min(njoy.cutoff_ev()) {
        let (a, b) = (njoy.inelastic_xs(e), ours.inelastic_xs(e));
        if a > 0.0 {
            let rel = b / a - 1.0;
            if rel.abs() > worst.abs() {
                worst = rel;
                worst_e = e;
            }
            n += 1;
        }
        e *= 1.05;
    }
    println!("{n} energies: worst sigma_inel difference {:+.3} % at {worst_e:.4e} eV", 100.0 * worst);
    assert!(worst.abs() <= 5.0e-3, "route-4 sigma_inel {:+.3} % from NJOY at {worst_e} eV", 100.0 * worst);
}
