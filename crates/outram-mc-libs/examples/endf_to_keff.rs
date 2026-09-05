//! End-to-end Monte Carlo k-eff from local ENDF files — a tutorial example.
//!
//! This example demonstrates the complete workflow for reading ENDF nuclear data
//! from disk files and running a k-effective (eigenvalue) calculation. It serves
//! as a first-time introduction to the `outram-mc-libs` API for users with local
//! ENDF tapes.
//!
//! **What this example does:**
//!
//! 1. **Reads ENDF files** from local disk using the new `Nuclide::from_endf_file()`
//!    function (no network download, no `net-fetch` feature required).
//!
//! 2. **Builds nuclides** — transportable nuclear data objects that hold continuous-energy
//!    cross sections and fission data, reconstructed from ENDF on device via RECONR
//!    (resonance reconstruction) and BROADR (Doppler broadening).
//!
//! 3. **Composes a material** from those nuclides, specifying isotope atom densities.
//!
//! 4. **Defines a bare sphere geometry** — the simplest case, used for critical-mass
//!    benchmarks like Godiva (a HEU metal sphere).
//!
//! 5. **Runs a fission-source power iteration** to find k-eff, the neutron multiplication
//!    factor. k-eff = 1.0 means critical (the fission chain is self-sustaining).
//!    k-eff < 1.0 means subcritical (chain dies out); k-eff > 1.0 is supercritical.
//!
//! 6. **Reports results** — the mean k-eff and its standard deviation across
//!    generations. The standard deviation shrinks as you run more histories; a smaller
//!    standard deviation means less Monte Carlo noise and higher confidence in the answer.
//!
//! **The example case:** A bare HEU (highly enriched uranium) sphere similar to the
//! ICSBEP Godiva benchmark: radius ≈ 8.741 cm, U-235 at 4.4994e-2 /barn-cm,
//! U-238 at 2.4984e-3 /barn-cm. The expected k-eff is close to 1.0.
//!
//! **How to run:**
//! ```text
//! cargo build --release -p outram-mc-libs --example endf_to_keff
//! cargo run --release -p outram-mc-libs --example endf_to_keff
//! ```
//!
//! ENDF files are read from `reference-data/endf/` relative to this crate's directory
//! (CARGO_MANIFEST_DIR). You may override this on the command line:
//! ```text
//! cargo run --release -p outram-mc-libs --example endf_to_keff -- /path/to/endf
//! ```
//!
//! **Data fidelity note:** This example uses RECONR (full resonance reconstruction at 0 K)
//! and BROADR (analytic Doppler broadening to 293.6 K). It reads energy-dependent cross
//! sections, anisotropic scattering (ENDF MF=4), and energy-dependent fission yields
//! (MF=5) — full continuous-energy physics, **with self-shielding** (σ sampled at the
//! actual neutron energy). This HIGH-tier fidelity is a step up from the embedded
//! LOW-tier (group-collapsed, infinite dilution); see the `godiva_keff` and
//! `godiva_keff_endf` examples for the methodology comparison.

use std::path::PathBuf;
// Everything this example needs comes from the prelude — that is the
// intended entry point, and it is worth checking that it suffices.
use outram_mc_libs::prelude::*;

fn main() {
    // ── Parse command-line arguments or use default. ─────────────────────────
    let endf_dir = if let Some(arg) = std::env::args().nth(1) {
        PathBuf::from(arg)
    } else {
        // Default: reference-data/endf/ relative to this crate's Cargo.toml.
        // CARGO_MANIFEST_DIR is set by cargo and points to the crate root.
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        PathBuf::from(manifest_dir)
            .join("..")
            .join("..")
            .join("reference-data")
            .join("endf")
    };

    println!("Reading ENDF files from: {}", endf_dir.display());

    // ── Step 1: Read ENDF files and build transport-ready nuclides. ──────────
    // The Nuclide::from_endf_file() function handles all the heavy lifting:
    //   - Reads the raw ENDF-formatted tape
    //   - Runs RECONR to reconstruct the resonance range (high-resolution cross sections)
    //   - Runs BROADR to Doppler-broaden the data to the material temperature (293.6 K here)
    //   - Extracts energy-dependent ν̄ (average neutrons per fission) from MF=1/452
    //
    // Arguments:
    //   - path:      filesystem path to the ENDF file
    //   - name:      human-readable nuclide name (e.g., "U235") for logging and diagnostics
    //   - temp_k:    material temperature in Kelvin; cross sections are broadened to this
    //   - tolerance: RECONR tolerance (e.g., 1e-3 = 0.1%); smaller = finer resonance grid
    //
    // This is a **local-file** counterpart to Nuclide::from_endf(), which downloads
    // from the IAEA and requires the net-fetch feature. Here, you provide the file yourself.

    let temp_k = 293.6; // Room temperature, K

    let u235_path = endf_dir.join("n-092_U_235-ENDF8.0.endf");
    let u238_path = endf_dir.join("n-092_U_238.endf");

    println!("\nReconstructing nuclides from ENDF (RECONR + BROADR @ {temp_k} K)…\n");

    let u235 = Nuclide::from_endf_file(&u235_path, "U235", temp_k, 1.0e-3)
        .expect("Could not read U-235 ENDF file");
    println!("  U-235: ✓");

    let u238 = Nuclide::from_endf_file(&u238_path, "U238", temp_k, 1.0e-3)
        .expect("Could not read U-238 ENDF file");
    println!("  U-238: ✓\n");

    // Store both nuclides in a vec. We'll reference them by index when building the material.
    let nuclides = vec![u235, u238];

    // ── Step 2: Build a material with specified isotope densities. ──────────
    // A Material is a mixture of nuclides at a given temperature and density.
    // The components list specifies which nuclide (by index into our `nuclides` vec)
    // and at what atom density [atoms / barn·cm].
    //
    // The HEU sphere (Godiva-like) composition:
    //   - U-235: 4.4994e-2 /barn·cm  (the fissile driver)
    //   - U-238: 2.4984e-3 /barn·cm  (adds absorption and fast fission, softens spectrum)

    let material = Material {
        id: 1,
        name: "HEU (highly enriched uranium)".into(),
        temperature: temp_k,
        components: vec![
            NuclideComponent {
                nuclide_idx: 0, // Points to nuclides[0], which is U-235
                atom_density: 4.4994e-2,
            },
            NuclideComponent {
                nuclide_idx: 1, // Points to nuclides[1], which is U-238
                atom_density: 2.4984e-3,
            },
        ],
    };

    // ── Step 3: Define geometry — a bare sphere. ────────────────────────────
    // The critical radius for a bare HEU sphere (ICSBEP HEU-MET-FAST-001) is
    // approximately 8.741 cm. The geometry here is implicit: run_keff() assumes
    // a spherical shell of material with the given radius, centered at the origin,
    // surrounded by vacuum. Neutrons leak when they cross the outer surface.
    // (For more complex geometries, see the geometry module and other examples.)

    let radius_cm = 8.741;

    // ── Step 4: Configure the Monte Carlo run. ────────────────────────────────
    // KeffSettings controls the fission-source power iteration:
    //   - n_particles:  number of neutron histories per generation (more ⇒ lower noise)
    //   - n_inactive:   generations to skip before tallying (let fission source converge)
    //   - n_active:     generations to average into the result
    //   - temperature_k: material/data temperature; cross-section lookups use this
    //   - seed:         RNG seed; fixed seed ⇒ reproducible result
    //   - compute:      which backend to use (CPU single/multi-thread, GPU); default is
    //                   single-thread, which is the trusted reference
    //
    // A full HIGH-fidelity run uses more histories; this tutorial keeps it modest
    // so it finishes quickly (~30 seconds on a laptop).

    let settings = KeffSettings {
        n_particles: 3000, // 3000 histories per generation
        n_inactive: 30,    // discard first 30 generations (source settling)
        n_active: 70,      // average the next 70 active generations
        temperature_k: temp_k,
        ..KeffSettings::default()
    };

    // ── Step 5: Run the k-effective calculation. ───────────────────────────────
    // This is the main "work" function: it launches the fission-source power
    // iteration. For each generation:
    //   1. Sample a fission source (neutrons born from previous generation's fissions)
    //   2. Transport each neutron through collisions in the HEU sphere
    //   3. Track fissions; estimate k = (neutrons from fissions) / (source neutrons)
    //   4. Use those fissions as the source for the next generation
    //   5. Converge to the true k-eff
    //
    // Returns KeffResult with k_mean (eigenvalue), k_std (standard error), and
    // per-generation tallies.

    println!(
        "Running k-eff calculation: {} histories/generation × [{}+{}] generations",
        settings.n_particles, settings.n_inactive, settings.n_active
    );
    println!("  r = {radius_cm} cm (bare HEU sphere)\n");

    let result = run_keff(radius_cm, &material, &nuclides, &settings);

    // ── Step 6: Report results. ───────────────────────────────────────────────
    // k_mean:  the estimated multiplication factor, averaged over active generations.
    //          Ideally close to 1.0 for Godiva (the benchmark reference is 1.0000 ± 0.0010).
    //
    // k_std:   the standard error (1-sigma uncertainty) in k_mean. Smaller is better.
    //          It shrinks with sqrt(n_histories) and sqrt(n_generations) — more histories
    //          = less Monte Carlo noise = lower standard deviation.
    //
    // The difference from the benchmark (Δk) is reported in pcm (parts per 100,000),
    // a standard unit in criticality analysis. A result within ~200 pcm of the
    // benchmark k-eff = 1.0000 is considered excellent agreement for a bare metal sphere.

    println!("Results:");
    println!("  k_eff = {:.5} ± {:.5}", result.k_mean, result.k_std);
    println!("  ICSBEP Godiva benchmark: k_eff = 1.0000 ± 0.0010");

    let delta_k_pcm = (result.k_mean - 1.0) * 1.0e5;
    println!("  Δk from benchmark = {delta_k_pcm:+.0} pcm");

    if (result.k_mean - 1.0).abs() < 0.005 {
        println!("\n  ✓ Result is within ~500 pcm of the benchmark. Good sanity check!");
    } else {
        println!(
            "\n  ⚠ Result is {:.0} pcm away from the benchmark. Check your ENDF files.",
            delta_k_pcm.abs()
        );
    }
}
