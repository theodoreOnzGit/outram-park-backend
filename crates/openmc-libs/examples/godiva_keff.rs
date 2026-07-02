//! Godiva bare-sphere criticality — the first end-to-end Keff in `openmc-libs`.
//!
//! Godiva is ICSBEP benchmark **HEU-MET-FAST-001**: a bare sphere of highly
//! enriched uranium metal, r ≈ 8.7407 cm, benchmark **k_eff = 1.0000 ± 0.0010**.
//! It is the canonical fast-spectrum criticality check.
//!
//! This example composes the whole transport stack — nuclear data pulled from
//! `njoy-outram-park-fork` (WMP + fast MGXS), the bare-sphere geometry, analog
//! collision physics, and fission-source power iteration — and prints the
//! converged eigenvalue. Cross sections ship *inside* the crates, so it runs with
//! no downloads and no HDF5.
//!
//! Run with:
//! ```text
//! cargo run --release -p openmc-libs --example godiva_keff
//! ```
//!
//! Fidelity caveat: infinite-dilution fast group data (no self-shielding) plus
//! analog isotropic-CM scatter — expect a first-cut result near, but not exactly
//! at, 1.0. See `docs/keff-doppler-roadmap.md`.

use openmc_libs::material::material::{Material, NuclideComponent};
use openmc_libs::material::nuclide::Nuclide;
use openmc_libs::physics::keff::{run_keff, KeffSettings};

fn main() {
    // ── Nuclear data: the three uranium isotopes in HEU metal. ────────────────
    let nuclides = vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
    ];

    // ── Material: Godiva atom densities [atoms/barn·cm] (HEU-MET-FAST-001). ────
    let material = Material {
        id: 1,
        name: "Godiva HEU".into(),
        temperature: 293.6, // K
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 }, // U-234
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 }, // U-235
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 }, // U-238
        ],
    };

    // ── Power iteration. ──────────────────────────────────────────────────────
    let settings = KeffSettings {
        n_particles: 5000,
        n_inactive: 40,
        n_active: 110,
        ..KeffSettings::default()
    };

    let radius_cm = 8.7407;
    println!("Godiva bare-sphere Keff  (r = {radius_cm} cm)");
    println!(
        "  {} histories/gen, {} inactive + {} active generations\n",
        settings.n_particles, settings.n_inactive, settings.n_active
    );

    let result = run_keff(radius_cm, &material, &nuclides, &settings);

    println!("  k_eff = {:.5} ± {:.5}", result.k_mean, result.k_std);
    println!("  ICSBEP benchmark = 1.0000 ± 0.0010");
    let pcm = (result.k_mean - 1.0) * 1.0e5;
    println!("  Δk from benchmark = {pcm:+.0} pcm");
}
