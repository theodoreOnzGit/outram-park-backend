//! `pincell` notebook → outram-mc verification (partial, LIVE).
//!
//! Notebook: `pincell.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Builds a single LWR fuel pin (UO2 fuel +
//! Zircaloy clad + borated water) inside a square cell with **reflective**
//! boundaries, runs a k-eigenvalue calculation, and tallies the cell flux.
//! OpenMC API exercised: `Material`, `ZCylinder`/`XPlane`/`YPlane`, `Universe`,
//! `Cell`, `Geometry`, `IndependentSource`, `Settings`, `run`,
//! `Tally`+`CellFilter`, `StatePoint`.
//!
//! **What outram-mc can do today.** The only working end-to-end path is
//! `physics::keff::run_keff` — a homogeneous **bare-sphere** k-eigenvalue power
//! iteration. That covers the notebook's *criticality-eigenvalue* core but not
//! its true model: the LWR pin needs a square reflected cell (general CSG +
//! reflective BC, gap op-6tz.7/.10), an S(alpha,beta) thermal water scatter
//! (gap op-6tz.12), and a transport-driven cell flux tally (gap op-6tz.9).
//!
//! So this file runs the natural first LIVE case — the **Godiva bare-sphere
//! k-eff** (ICSBEP HEU-MET-FAST-001, op-u6s.1) — as the criticality-eigenvalue
//! verification, and marks the LWR-specific pieces as ignored gaps. This is an
//! honest reduced slice, not the notebook's LWR pin cell.
//!
//! # V&V — methodology and results
//!
//! **Methodology.** Bare HEU sphere, r = 8.7407 cm, ICSBEP HEU-MET-FAST-001
//! atom densities (U-234/235/238), 1500 histories x [20 inactive + 40 active],
//! embedded LOW-tier cross sections (`Nuclide::from_core`: WMP below each
//! nuclide's e_max + Watt-collapsed fast MGXS with per-group mean cosine and
//! carved inelastic above). Reference: ICSBEP k_eff = 1.0000 +/- 0.0010. Pass
//! criterion is a **broad plausibility band** [0.9, 1.4] with sigma < 0.02 — it
//! guards the whole transport chain (data -> geometry -> collision -> scatter ->
//! fission -> power iteration), not benchmark accuracy.
//!
//! **Results (2026-07-15, this harness).** Recorded by the assertions below at
//! run time; the LOW-tier Godiva model lands near k_eff ~ 1.010 +/- 0.002
//! (~+1000 pcm), consistent with the `src/physics/keff.rs` unit-test result and
//! the documented fidelity caveats (no self-shielding; one mean cosine;
//! Weisskopf evaporation for inelastic). Numbers here are asserted, not
//! hard-coded, so a regression trips the band rather than a stale literal.

use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{run_keff, KeffSettings};

/// Godiva atom densities (HEU-MET-FAST-001), atoms/barn-cm.
fn godiva_material() -> Material {
    Material {
        id: 1,
        name: "Godiva HEU".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 }, // U-234
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 }, // U-235
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 }, // U-238
        ],
    }
}

/// LIVE: criticality-eigenvalue slice of the `pincell` notebook, realised as the
/// Godiva bare-sphere k-eff (op-u6s.1). Verifies the transport chain converges
/// to a stationary, plausible eigenvalue. See the module V&V note.
#[test]
fn pincell_criticality_eigenvalue_via_godiva_bare_sphere() {
    let nuclides = vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
    ];
    let material = godiva_material();
    let settings = KeffSettings {
        n_particles: 1500,
        n_inactive: 20,
        n_active: 40,
        ..KeffSettings::default()
    };

    let result = run_keff(8.7407, &material, &nuclides, &settings);

    assert_eq!(
        result.k_by_generation.len(),
        settings.n_inactive + settings.n_active,
        "power iteration ran every generation"
    );
    assert!(
        result.k_mean > 0.9 && result.k_mean < 1.4,
        "Godiva k_eff {} outside the plausible first-cut band [0.9, 1.4]",
        result.k_mean
    );
    assert!(
        result.k_std < 0.02,
        "k noisy/unconverged: sigma = {}",
        result.k_std
    );
}

/// LIVE sign check: a far-subcritical (tiny, leakage-dominated) sphere must be
/// less reactive than the near-critical Godiva sphere. Confirms the
/// geometry/leakage coupling actually bites — the same leakage physics the
/// pincell notebook's reflective boundaries would suppress.
#[test]
fn pincell_leakage_reduces_reactivity() {
    let nuclides = vec![Nuclide::from_core("U235").unwrap()];
    let material = Material {
        id: 1,
        name: "U235".into(),
        temperature: 293.6,
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 4.8e-2 }],
    };
    let settings =
        KeffSettings { n_particles: 1000, n_inactive: 15, n_active: 25, ..KeffSettings::default() };

    let k_big = run_keff(9.0, &material, &nuclides, &settings).k_mean;
    let k_small = run_keff(3.0, &material, &nuclides, &settings).k_mean;
    assert!(
        k_small < k_big,
        "3 cm sphere (k={k_small}) should leak more than 9 cm (k={k_big})"
    );
}

/// GAP: the notebook's actual LWR pin — UO2/Zr/borated-water pin in a square
/// **reflective** cell with an S(alpha,beta) thermal spectrum and a cell flux
/// tally. Needs general CSG navigation + reflective BC (op-6tz.7/.10),
/// S(alpha,beta) thermal scatter in transport (op-6tz.12), and tally scoring
/// (op-6tz.9). None exist today.
#[test]
#[ignore = "requires general CSG + reflective BC + S(a,b) + tally scoring (op-6tz.7/.10/.12/.9)"]
fn pincell_lwr_reflected_pin_full_model() {
    unimplemented!(
        "LWR pin-in-reflected-cell with thermal spectrum + cell flux tally: op-6tz.7/.10/.12/.9"
    );
}
