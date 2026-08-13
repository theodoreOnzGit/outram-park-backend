//! `search` notebook → outram-mc verification (LIVE, criticality-search driver).
//!
//! Notebook: `search.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! # What the notebook does
//!
//! The OpenMC `search.ipynb` drives `openmc.search_for_keff` — a **bisection**
//! criticality search — over the soluble-boron concentration \[ppm\] of a PWR
//! UO₂ pin cell (fuel + Zircaloy clad + borated light water, reflective
//! boundaries), root-finding the boron loading that makes the lattice exactly
//! critical. The notebook's reported result is a **critical boron concentration
//! of ≈ 1926 ppm** (its printed search converges there; more boron ⇒ more
//! thermal absorption ⇒ lower `k`, a monotone-decreasing knob). OpenMC API
//! exercised: `openmc.search_for_keff(model_builder, bracket=..., tol=...,
//! bracketed_method='bisect')`, `Model`, `Material`, k-eigenvalue `run`.
//!
//! # What outram-mc realises here (op-6tz.6.5)
//!
//! The **search driver itself** — the capability bead op-6tz.6.5 assigns to
//! `outram-mc-libs` (njoy only supplies cross sections) — is now implemented:
//! [`outram_mc_libs::physics::search::search_for_keff`], a bracketed bisection /
//! false-position root-find of a scalar parameter `p` such that `k_eff(p) =
//! target`, taking a closure `p -> KeffResult`. It mirrors OpenMC's
//! `bracketed_method='bisect'` semantics (sign-based interval halving, robust to
//! Monte Carlo noise). The stub in
//! `njoy-outram-park-fork/tests/openmc_notebooks_data/search.rs` deferred exactly
//! this to the transport crate.
//!
//! # HONEST DATA CAVEAT — why the offline analogue, not the boron ppm case
//!
//! The notebook's **exact** PWR boron-ppm case is **data-gated offline in this
//! environment**: a borated-water PWR pin cell only goes critical *thermally*,
//! which requires H-in-H₂O **S(α,β)** thermal-scattering data
//! (`tsl-HinH2O.endf`). That file is a large public ENDF/B-VIII.0 dataset that is
//! not vendored here (see `DATA_POLICY.md`); the sibling thermal pin-cell test
//! (`pincell::pincell_lwr_thermal_pin_benchmark`) is itself data-gated on it and
//! SKIPs when it is absent. So the exact 1926-ppm boron search cannot be run
//! offline. Rather than fake it, this test verifies the **search driver** on an
//! **offline-tractable analogue** that uses only embedded CORE data
//! (`Nuclide::from_core`, no network, no S(α,β)):
//!
//! > **Search for the critical radius of a bare HEU (Godiva) sphere** — find the
//! > radius `r` \[cm\] where `k_eff(r) = 1`, using the same bisection driver the
//! > notebook uses for boron ppm. Here the monotone knob is the sphere radius
//! > (bigger ⇒ less leakage ⇒ higher `k`), the *opposite* orientation to boron,
//! > which also exercises the driver's orientation-free sign handling.
//!
//! # V&V — methodology and results
//!
//! **Methodology.** Fission-source power iteration
//! ([`outram_mc_libs::physics::keff::run_keff`], bare-sphere analog transport,
//! LOW-tier CORE data: WMP below `e_max` + Watt-collapsed fast MGXS for
//! U-234/235/238 at Godiva HEU-MET-FAST-001 atom densities) is wrapped in a
//! closure `r -> KeffResult` and handed to
//! [`outram_mc_libs::physics::search::search_for_keff`] with a bracket of
//! `[7.5, 10.0]` cm, `target = 1.0`, `method = Bisect`, parameter tolerance
//! `0.2` cm, fixed seed (deterministic). Each bisection step is one full
//! k-eigenvalue solve (1500 histories, 20 inactive + 40 active).
//!
//! **Reference.** The bare-sphere critical radius of Godiva (highly-enriched-U
//! metal sphere) is **r_c ≈ 8.741 cm** — the ICSBEP benchmark **HEU-MET-FAST-001**
//! (the same geometry the `keff.rs` / `pincell` tests use). This is the analogue's
//! reference; the notebook's own reference (≈ 1926 ppm boron) is cited above for
//! the *method*, which is what this test verifies.
//!
//! **Results (measured 2026-08-06, this environment, seed 12345, LOW-tier CORE
//! data).** The measured bisection trajectory brackets the k = 1 crossing between
//! r = 8.4375 cm (k = 0.97748 ± 0.00562) and r = 8.75 cm (k = 1.01188 ± 0.00492).
//! The driver's measured outputs (also printed via `eprintln!`; asserted, not
//! hard-coded):
//!
//! - **Bisection** (`method = Bisect`, radius tol 0.2 cm, bracket [7.5, 10.0]):
//!   converged **r_c = 8.5938 cm**, **k = 0.99281 ± 0.00523**, in **6
//!   k-eigenvalue solves** (2 endpoints + 4 bisection steps). Trajectory
//!   (r → k ± σ): 7.5 → 0.88787 ± 0.00458; 10.0 → 1.12716 ± 0.00547;
//!   8.75 → 1.01188 ± 0.00492; 8.125 → 0.95381 ± 0.00609;
//!   8.4375 → 0.97748 ± 0.00562; 8.5938 → 0.99281 ± 0.00523.
//! - **False position** (`method = Secant`, radius tol 0.25 cm): converged
//!   **r_c = 8.5600 cm**, **k = 0.99326 ± 0.00588**.
//!
//! Both land at **r_c ≈ 8.56–8.59 cm**, **≈ 0.15–0.18 cm (1.7–2.1 %) below** the
//! ICSBEP reference of 8.741 cm. That offset is the **known LOW-tier fast-data
//! bias** (this embedded WMP+MGXS data gives k at the ICSBEP radius ≈ 1.010 —
//! `examples/godiva_keff` measured k = 1.01042 ± 0.00174 at r = 8.7407 cm on
//! 2026-08-06, marginally supercritical, so the k = 1 crossing sits slightly
//! low), **not** a driver error — the driver
//! reproduces the analytic root of a clean synthetic monotone model to machine
//! tolerance in its unit tests (`physics::search::tests`). Interpretation: the
//! `search_for_keff` bisection/false-position criticality-search driver is
//! verified end-to-end against a real transport k-eigenvalue solve, converging to
//! a physical critical dimension consistent with a published benchmark within the
//! data tier's known bias. The exact 1926-ppm boron notebook case remains
//! data-gated offline (see the caveat above).
//!
//! **Supersedes (pre-`op-jis` figures, measured 2026-07-23).** The values above
//! replace ones taken with the old `prn` output function (uniforms formed from
//! the raw top-52 state bits, before bead `op-jis` added OpenMC's PCG-RXS-M-XS
//! output permutation). The LCG *state* recurrence is unchanged — so the seed and
//! the bisection's radius sequence are untouched — but every sampled uniform
//! moved, and with it every k. Superseded:
//! - Calibration points: **k(8.5 cm) = 0.98480 ± 0.00471** and
//!   **k(8.741 cm) = 1.01136 ± 0.00461**. Neither radius was re-run on
//!   2026-08-06 (no test evaluates them), so **both need a re-run**; the new
//!   crossing bracket quoted above comes from the measured bisection trajectory
//!   instead, and no replacement value is invented for these two radii.
//! - **Bisection**: r_c = 8.5938 cm, k = 0.99564 ± 0.00603 (6 solves; same radius
//!   trajectory).
//! - **False position**: r_c = 8.6112 cm, k = 0.99448 ± 0.00416.
//! - Summary offset: "both land at r_c ≈ 8.60 cm, ≈ 0.14 cm (1.6 %) below 8.741
//!   cm", and "k(8.741) ≈ 1.01" from the pre-`op-jis` data.

use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
use outram_mc_libs::physics::search::{search_for_keff, SearchMethod, SearchSettings};

/// Godiva HEU atom densities (ICSBEP HEU-MET-FAST-001), atoms/barn-cm — the same
/// material the `pincell` and `keff` bare-sphere tests use.
fn godiva_material() -> Material {
    Material {
        id: 1,
        name: "Godiva HEU".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.9184e-4,
            }, // U-234
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 4.4994e-2,
            }, // U-235
            NuclideComponent {
                nuclide_idx: 2,
                atom_density: 2.4984e-3,
            }, // U-238
        ],
    }
}

/// The three uranium nuclides for Godiva, all from the embedded CORE WMP library
/// (`Nuclide::from_core`, offline — no ENDF file, no network).
fn godiva_nuclides() -> Vec<Nuclide> {
    vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
    ]
}

/// LIVE (op-6tz.6.5): the `search` notebook's **`search_for_keff` bisection
/// driver**, verified on the offline Godiva-critical-radius analogue.
///
/// See the module docs for the full methodology, the honest data caveat (the
/// exact boron-ppm PWR case is thermal-S(α,β)-data-gated offline), and the ICSBEP
/// r_c ≈ 8.741 cm reference. This asserts the driver converges to a physical
/// critical radius near that reference with k ≈ 1 at convergence.
#[test]
fn search_for_critical_radius_godiva_bisection() {
    let nuclides = godiva_nuclides();
    let material = godiva_material();
    let keff_settings = KeffSettings {
        n_particles: 1500,
        n_inactive: 20,
        n_active: 40,
        seed: 12345,
        ..KeffSettings::default()
    };

    // Model closure: radius [cm] -> k-eigenvalue solve. This is the OUTRAM PARK
    // equivalent of the notebook's `build_model(boron_ppm)`.
    let mut n_solves = 0usize;
    let k_of_r = |r: f64| {
        n_solves += 1;
        run_keff(r, &material, &nuclides, &keff_settings)
    };

    // Bisection, mirroring the notebook's bracketed_method='bisect'. Parameter
    // tolerance is on the radius [cm]; k_tol disabled so the run is a clean,
    // deterministic bisection to the radius tolerance.
    let settings = SearchSettings {
        target: 1.0,
        tol: 0.2,
        k_tol: 0.0,
        max_iterations: 20,
        method: SearchMethod::Bisect,
    };

    let result = search_for_keff(k_of_r, (7.5, 10.0), &settings).expect("bracket straddles k = 1");

    eprintln!(
        "[search] critical radius = {:.4} cm, k = {:.5} ± {:.5}  \
         (converged = {}, {} k-eigenvalue solves)",
        result.parameter, result.keff, result.keff_std, result.converged, n_solves
    );
    for it in &result.iterations {
        eprintln!(
            "    r = {:>7.4} cm  ->  k = {:.5} ± {:.5}",
            it.parameter, it.keff, it.keff_std
        );
    }

    // The driver reached a convergence criterion (radius tolerance), not the cap.
    assert!(
        result.converged,
        "criticality search did not converge within the iteration budget"
    );

    // Converged radius brackets the ICSBEP Godiva reference (8.741 cm). LOW-tier
    // CORE data lands slightly low (~8.6 cm), so a band around the reference.
    assert!(
        result.parameter > 8.0 && result.parameter < 9.3,
        "critical radius {} cm outside the physical band [8.0, 9.3] around ICSBEP r_c = 8.741 cm",
        result.parameter
    );

    // k at the converged radius must be near critical: within 3σ of unity (σ is
    // the per-solve k standard error, ~0.005 here).
    let tol_k = 3.0 * result.keff_std.max(0.005);
    assert!(
        (result.keff - 1.0).abs() < tol_k,
        "k at converged radius = {} (± {}) not within 3σ of critical (1.0)",
        result.keff,
        result.keff_std
    );

    // The trajectory records both endpoints plus every midpoint (>= 3 points).
    assert!(
        result.iterations.len() >= 3,
        "search trajectory too short: {:?}",
        result.iterations
    );
}

/// LIVE cross-check: the **bracket-preserving secant (false position)** method
/// of the same driver reaches a consistent critical radius on the identical
/// Godiva analogue — verifying the alternate root-finder, not a new physics case.
///
/// Uses a coarser radius tolerance (fewer solves) since false position converges
/// faster than bisection; asserts it lands in the same physical band as the
/// bisection test above.
#[test]
fn search_for_critical_radius_godiva_secant() {
    let nuclides = godiva_nuclides();
    let material = godiva_material();
    let keff_settings = KeffSettings {
        n_particles: 1500,
        n_inactive: 20,
        n_active: 40,
        seed: 12345,
        ..KeffSettings::default()
    };

    let settings = SearchSettings {
        target: 1.0,
        tol: 0.25,
        k_tol: 0.0,
        max_iterations: 20,
        method: SearchMethod::Secant,
    };

    let result = search_for_keff(
        |r| run_keff(r, &material, &nuclides, &keff_settings),
        (7.5, 10.0),
        &settings,
    )
    .expect("bracket straddles k = 1");

    eprintln!(
        "[search/secant] critical radius = {:.4} cm, k = {:.5} ± {:.5} (converged = {})",
        result.parameter, result.keff, result.keff_std, result.converged
    );

    assert!(result.converged, "secant search did not converge");
    assert!(
        result.parameter > 7.8 && result.parameter < 9.5,
        "secant critical radius {} cm outside the physical band around 8.741 cm",
        result.parameter
    );
}
