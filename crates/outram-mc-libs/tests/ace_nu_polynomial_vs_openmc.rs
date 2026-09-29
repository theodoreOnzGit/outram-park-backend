// SPDX-License-Identifier: GPL-3.0

//! **A polynomial nu-bar (ACE NU `LNU = 1`) is evaluated exactly, at every
//! energy, as OpenMC evaluates it** — GitHub #365 audit.
//!
//! # What was wrong
//!
//! `acer::ce_laws::decode_nu` tabulated a polynomial nu-bar on a 1.2-ratio
//! grid over 1e-5 eV to 20 MeV and `NuBar::at` read it lin-lin, clamped at the
//! ends. So it was approximate inside the range (a quadratic is not piecewise
//! linear) and frozen above 20 MeV. OpenMC's `Polynomial`
//! (`openmc/data/reaction.py:263-268`) is evaluated exactly and unclamped.
//! `NuBar::poly` now carries the coefficients in eV powers on both routes
//! (ENDF `LNU = 1` too).
//!
//! # Methodology
//!
//! No held evaluation uses `LNU = 1`, so
//! `verification_and_validation/ace_route_physics/openmc_inputs/nu_polynomial_reference.py`
//! builds one. It appends a quadratic NU block to NJOY2016's U-235 table,
//! `nu = 2.4355 + 0.0650 E + 0.0020 E^2` with E in MeV, into
//! `target/ace_extra/U235_polynu.ace`, and records OpenMC's reading of it at
//! ten energies from 1e-7 eV to 150 MeV (`data/nu_polynomial_openmc.csv`).
//! `Nuclide::nu_bar` on the same file must equal OpenMC's value to 1e-12
//! relative. Both evaluate the same polynomial in double precision, so a
//! larger difference would be an error in the port. The test skips when the
//! file is absent.
//!
//! # Results (2026-09-29)
//!
//! All ten energies agree to the printed precision, including 30 MeV
//! (6.1855) and 150 MeV (57.1855), where the old clamp gave the 20 MeV value.

use outram_mc_libs::material::nuclide::Nuclide;
use std::path::PathBuf;

#[test]
fn polynomial_nu_matches_openmc_at_every_energy() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ace = root.join("../../target/ace_extra/U235_polynu.ace");
    if !ace.is_file() {
        println!("{} absent: skipping", ace.display());
        return;
    }
    let nuc = Nuclide::from_ace_file(&ace, "U235").expect("from_ace_file");
    let csv =
        root.join("verification_and_validation/ace_route_physics/data/nu_polynomial_openmc.csv");
    let text = std::fs::read_to_string(csv).expect("reference");
    let mut n = 0;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let v: Vec<f64> = line.split(',').map(|x| x.parse().unwrap()).collect();
        let got = nuc.nu_bar(v[0]);
        let rel = ((got - v[1]) / v[1]).abs();
        println!(
            "E = {:.3e} eV: nu = {got:.12} (OpenMC {:.12}), rel {rel:.1e}",
            v[0], v[1]
        );
        assert!(rel < 1.0e-12, "E = {}: {got} vs OpenMC {}", v[0], v[1]);
        n += 1;
    }
    assert_eq!(n, 10);
}
