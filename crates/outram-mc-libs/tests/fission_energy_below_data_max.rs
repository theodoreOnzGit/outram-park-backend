// SPDX-License-Identifier: GPL-3.0

//! **GitHub #463 item 2: no fission neutron is born at or above the top of
//! the data.** OpenMC's `sample_fission_neutron` resamples until
//! `E < data::energy_max[neutron]` (`src/physics.cpp:1091-1109`).
//!
//! OpenMC's bound is the lowest top energy of any loaded nuclide
//! (`library_energy_max_ev`), passed by every transport kernel.
//!
//! # Methodology
//!
//! U-235 from the committed NJOY2016 ACE table at 293.6 K
//! (`reference-data/ace`), 10^6 births per arm. Pass criteria, fixed before
//! running:
//!
//! 1. At an incident energy just below U-235's own top (30 MeV), every birth
//!    is below that top. (Measured 2026-09-30: the highest of 10^6 births is
//!    23.0 MeV, so this arm cannot fail on U-235 alone; it is kept as the
//!    no-cap statement.)
//! 2. With a cap of 10 MeV (standing in for a library whose lowest top is
//!    10 MeV) at 20 MeV incident, every birth is below the cap, **and** the
//!    uncapped draw puts some births above it, so the arm is able to fail.
//!    Before 2026-09-30 nothing enforced the cap.

use outram_mc_libs::material::nuclide::Nuclide;

#[test]
fn fission_births_stay_below_the_data_maximum() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/U235.ace.gz");
    if !p.exists() {
        eprintln!("SKIP: reference-data/ace submodule not initialised");
        return;
    }
    let u235 = Nuclide::from_ace_file(&p, "U235").expect("ACE table decodes");
    let e_top = u235.pointwise_energy_max_ev().expect("an ACE nuclide is pointwise");
    let e_in = e_top * (1.0 - 1.0e-9);
    let mut seed = 20260930_u64;
    let n = 1_000_000;
    let mut max_seen = 0.0_f64;
    for _ in 0..n {
        let e = u235.sample_fission_energy(e_in, &mut seed);
        max_seen = max_seen.max(e);
        assert!(e < e_top, "fission neutron born at {e:.6e} eV, at or above the data top {e_top:.6e} eV");
    }
    println!("U-235 data top {e_top:.4e} eV; highest of {n} births {max_seen:.4e} eV");

    let cap = 1.0e7;
    let e_in = 2.0e7;
    let mut above_uncapped = 0usize;
    for _ in 0..n {
        let e = u235.sample_fission_energy_below(e_in, cap, &mut seed);
        assert!(e < cap, "fission neutron born at {e:.6e} eV, at or above the cap {cap:.1e} eV");
        if u235.sample_fission_energy(e_in, &mut seed) >= cap {
            above_uncapped += 1;
        }
    }
    println!("uncapped: {above_uncapped} of {n} births at or above {cap:.1e} eV at {e_in:.1e} eV incident");
    assert!(above_uncapped > 0, "no uncapped birth reached the cap: this arm cannot fail");
}
