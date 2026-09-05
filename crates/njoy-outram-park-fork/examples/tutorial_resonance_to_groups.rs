//! # Nuclear-Data Processing Pipeline: Resonance Self-Shielding and Doppler Effects
//!
//! This tutorial demonstrates the complete workflow a reactor physicist uses when
//! preparing nuclear data for deterministic transport codes:
//!
//! 1. **Read an ENDF evaluation** — the standardized file format containing
//!    experimental resonance parameters and cross-section data
//! 2. **Reconstruct pointwise cross sections (RECONR)** — convert the compact
//!    resonance representation into an energy-dependent grid suitable for
//!    interpolation
//! 3. **Doppler broaden to multiple temperatures (BROADR)** — model how thermal
//!    motion of nuclei shifts resonance peaks, changing absorption rates
//! 4. **Extract physically meaningful metrics** — compute the **resonance integral**
//!    (a key quantity for reactor design) and show how it depends on temperature
//!
//! ## Why This Matters: Self-Shielding and the Resonance Integral
//!
//! **The Problem:** At room temperature, U-238 has razor-sharp resonances in neutron
//! capture (the (n,γ) reaction, MT=102). Deep in these resonances, the microscopic
//! cross section σ_γ can reach thousands of barns. However:
//! - In a real reactor, hundreds of U-238 nuclei sit between the neutron source and
//!   a measurement point. The strong resonances absorb many neutrons, so fewer
//!   neutrons *at the resonance energy* reach deeper into the material.
//! - The neutrons that DO penetrate tend to be at energies where σ_γ is lower
//!   (between peaks or at resonance tails).
//! - This **self-shielding** reduces the effective absorption rate compared to an
//!   isolated nucleus.
//!
//! **The Doppler Effect:** When temperature increases:
//! - Nuclei jiggle around their lattice sites (thermal motion).
//! - A stationary neutron *appears* to have different energy to a moving nucleus
//!   (Doppler shift).
//! - The result: resonance peaks broaden (lower amplitude, wider) and fill in the
//!   valleys between them.
//! - **Broadened peaks capture fewer neutrons** (less steep, less height) — the
//!   resonance integral *decreases* with temperature.
//!
//! This is the **negative feedback** that makes nuclear reactors safe: when a
//! reactor warms up, less neutron absorption at U-238 resonances, so fewer
//! neutrons are lost, releasing more heat, causing further cooling — a
//! self-regulating system.
//!
//! ## What You'll See
//!
//! - The **U-238 capture resonance integral**, integrated from ~4 eV to 100 keV,
//!   at room temperature (~294 K), measured values, and elevated temperatures.
//! - Published reference value: **~275 barns** at standard neutron-flux conditions.
//! - How the integral shrinks as temperature increases (the temperature coefficient
//!   of reactivity!).
//! - A side-by-side table showing the effect is real and quantifiable.

use njoy_outram_park_fork::prelude::*;
use std::path::PathBuf;

/// Path to the default U-238 ENDF file, relative to the crate root.
fn default_u238_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../reference-data/endf/n-092_U_238.endf");
    p
}

/// Integration bounds for the resonance integral [eV].
/// 4 eV to 100 keV is the canonical "resolved resonance range" for U-238.
const RESONANCE_INTEGRAL_LOW_EV: f64 = 4.0;
const RESONANCE_INTEGRAL_HIGH_EV: f64 = 1.0e5;

/// Calculate the integral of a cross section over an energy range using
/// piecewise linear (trapezoidal) integration.
/// Assumes `pairs` is sorted by energy.
fn integrate_cross_section(pairs: &[(f64, f64)], e_min: f64, e_max: f64) -> f64 {
    // Filter to energy range and guard against empty result
    let in_range: Vec<(f64, f64)> = pairs
        .iter()
        .filter(|(e, _)| *e >= e_min && *e <= e_max)
        .copied()
        .collect();

    if in_range.len() < 2 {
        // If fewer than 2 points in range, no meaningful integration
        return 0.0;
    }

    // Trapezoidal rule: sum of (E[i+1] - E[i]) * (σ[i+1] + σ[i]) / 2
    let mut integral = 0.0;
    for i in 0..in_range.len() - 1 {
        let (e1, s1) = in_range[i];
        let (e2, s2) = in_range[i + 1];
        integral += (e2 - e1) * (s1 + s2) / 2.0;
    }

    integral
}

/// Print a formatted table of temperature vs. resonance integral.
fn print_resonance_integral_table(
    results: &[(f64, f64)], // (temperature [K], resonance integral [barns])
) {
    let line = "═".repeat(70);
    let dash = "─".repeat(70);

    println!("\n{}", line);
    println!("U-238 Capture Resonance Integral vs. Temperature");
    println!("{}", line);
    println!(
        "{:>15} {:>20} {:>25}",
        "Temperature (K)", "Res. Integral (b)", "% Change from 0 K"
    );
    println!("{}", dash);

    let integral_at_0k = results
        .first()
        .map(|(_, integral)| *integral)
        .unwrap_or(0.0);

    for (temp, integral) in results {
        let pct_change = if integral_at_0k > 0.0 {
            ((integral - integral_at_0k) / integral_at_0k) * 100.0
        } else {
            0.0
        };
        println!(
            "{:>15.1} {:>20.6} {:>24.2}%",
            temp, integral, pct_change
        );
    }
    println!("{}", line);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ─────────────────────────────────────────────────────────────────────────
    // STEP 0: Load the ENDF file
    // ─────────────────────────────────────────────────────────────────────────

    let endf_path = if let Some(arg) = std::env::args().nth(1) {
        PathBuf::from(arg)
    } else {
        default_u238_path()
    };

    let line = "═".repeat(70);
    println!("{}", line);
    println!("Tutorial: U-238 Resonance Integral and Doppler Broadening");
    println!("{}", line);
    println!("\nLoading ENDF file: {}", endf_path.display());

    if !endf_path.exists() {
        eprintln!(
            "ERROR: ENDF file not found at {}",
            endf_path.display()
        );
        std::process::exit(1);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // STEP 1: Parse the ENDF evaluation
    // ─────────────────────────────────────────────────────────────────────────
    //
    // The ENDF-6 format stores nuclear data in "files" (MF) and "sections" (MT):
    //
    // - **MF=1:** General information (ZA, AWR, mass, resonance parameters present?)
    // - **MF=2:** Resonance parameters (at 0 K: Breit-Wigner, R-matrix, etc.)
    // - **MF=3:** Smooth cross-section background (below resolved resonances)
    // - **MF=7+:** Thermal scattering data (S(α,β), angular distributions, etc.)
    //
    // For U-238, MF=2 contains resolved resonances from ~0.4 eV to ~25 keV.
    // At 0 K, these are infinitesimally sharp; BROADR broadens them.

    let tape = Tape::read_file(&endf_path)?;
    println!("✓ Parsed ENDF tape with {} sections", tape.sections().len());

    // Find the material number (most ENDF files have one primary material)
    let materials = tape.materials();
    let mat = *materials.first().ok_or("No materials found in ENDF tape")?;

    println!("  Material MAT = {}", mat);

    // ─────────────────────────────────────────────────────────────────────────
    // STEP 2: Reconstruct pointwise cross sections at 0 K (RECONR)
    // ─────────────────────────────────────────────────────────────────────────
    //
    // RECONR converts the compact ENDF format (a handful of resonance parameters)
    // into a fine **pointwise grid** of energies with corresponding cross sections.
    //
    // At 0 K (the thermal-motion reference):
    // - Resonance peaks are infinitesimally sharp (Dirac deltas in the mathematical
    //   limit, but RECONR resolves them to machine precision).
    // - The grid is **lin-lin** interpolated: linear in energy, linear in σ(E).
    // - A tolerance parameter (here 0.001 = 0.1%) controls linearization error.
    //
    // The output ReconrResult contains:
    // - `material`: ZA, AWR, E_max, other header data
    // - `sections`: one ReconrSection per reaction (MT=1 total, MT=2 elastic,
    //   MT=102 capture, etc.), each with `pairs: Vec<(E, σ)>` sorted by E.

    let config_0k = ReconrConfig {
        mat,
        tolerance: 0.001, // Match NJOY's typical default
        temperature: 0.0, // 0 K: no Doppler shift
    };

    println!("\nRunning RECONR at 0 K...");
    let reconr_0k = reconr(&tape, &config_0k)?;

    println!(
        "✓ Reconstructed {} reactions at 0 K",
        reconr_0k.sections.len()
    );
    println!(
        "  Atomic weight ratio (AWR): {:.6} (mass/n_mass)",
        reconr_0k.material.awr
    );
    println!("  Max evaluation energy:     {:.3e} eV", reconr_0k.material.emax);

    // ─────────────────────────────────────────────────────────────────────────
    // STEP 3: Compute resonance integrals at multiple temperatures via Doppler
    // broadening (BROADR)
    // ─────────────────────────────────────────────────────────────────────────
    //
    // Doppler broadening models the effect of thermal motion. The BROADR module:
    //
    // 1. Takes the 0 K pointwise grid (from RECONR above).
    // 2. For each output energy E_out, integrates the collision kernel (the
    //    probability that a neutron at E_in, after colliding with a nucleus
    //    moving at temperature T, appears to have energy E_out).
    // 3. Uses the free-gas model: nucleus recoil is unbound, not in a lattice.
    // 4. Implements the SIGMA1 method: analytic f-functions for the integral.
    //
    // Result: same energy grid (in eV), but with σ values convolved with
    // thermal motion. At higher T, resonance peaks broaden (wider, lower).
    //
    // **Key physics:** The narrower, sharper a resonance, the more neutrons
    // *skip through* without being absorbed as temperature rises. Hence the
    // resonance integral drops, and fewer neutrons are captured. This is the
    // **negative temperature coefficient** that keeps reactors safe.

    let temperatures_k = vec![
        0.0,    // Theoretical 0 K (for comparison)
        293.6,  // Room temperature (20°C)
        600.0,  // Moderately elevated
        900.0,  // Reactor-relevant
        1200.0, // High temperature
    ];

    println!("\nDoppler-broadening cross sections at {} temperatures...", temperatures_k.len());
    let mut resonance_integrals: Vec<(f64, f64)> = Vec::new();

    for temp_k in &temperatures_k {
        // For 0 K, use the 0 K RECONR result directly (no broadening).
        let sections_at_temp = if *temp_k == 0.0 {
            reconr_0k.sections.clone()
        } else {
            // Use BROADR to apply Doppler broadening.
            doppler_broaden(&reconr_0k.sections, reconr_0k.material.awr, *temp_k)
        };

        // Find the capture cross section (MT=102).
        let capture_xs = sections_at_temp
            .iter()
            .find(|s| s.mt.number() == 102)
            .ok_or("MT=102 (capture) not found in RECONR result")?;

        // Compute the resonance integral over [4 eV, 100 keV].
        let res_integral = integrate_cross_section(
            &capture_xs.pairs,
            RESONANCE_INTEGRAL_LOW_EV,
            RESONANCE_INTEGRAL_HIGH_EV,
        );

        resonance_integrals.push((*temp_k, res_integral));

        println!("  T = {:7.1} K  → RI = {:8.4} barns", temp_k, res_integral);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // STEP 4: Display results and physical interpretation
    // ─────────────────────────────────────────────────────────────────────────

    print_resonance_integral_table(&resonance_integrals);

    let line = "═".repeat(70);
    println!("\n{}", line);
    println!("Physical Interpretation");
    println!("{}", line);

    println!(
        "\n1. **Resonance Integral Definition:**
   The resonance integral (RI) is the energy-weighted absorption rate,
   integrated over a material at infinite dilution (no self-shielding).

   RI = ∫₄ₑᵥ¹⁰⁰ₖₑᵥ σ_γ(E, T) dE

   Units: barns (1 barn = 10⁻²⁴ cm²). Historical reference: ~275 b for U-238
   at standard conditions."
    );

    println!(
        "\n2. **Doppler Broadening:**
   At 0 K, neutrons see infinitesimally sharp resonance peaks.
   At finite T, thermal motion of the U-238 nucleus causes the resonance
   to broaden (wider in energy, lower in amplitude). This is the
   **Doppler effect**, not to be confused with the Doppler shift of light.

   - Narrower peaks → fewer neutrons absorbed at resonance energy
   - Valleys fill in partially → absorption off-resonance increases,
     but less than the loss at peak
   - Net effect: RI decreases with T (negative temperature coefficient)."
    );

    println!(
        "\n3. **Reactor Safety Implication (Negative Feedback):**
   If reactor power increases → temperature rises → resonance peaks broaden
   → fewer neutrons absorbed by U-238 resonances → more neutrons leak away
   or cause fission → if neutron production < loss, power decreases (feedback).
   This self-regulating mechanism is crucial for passive safety."
    );

    println!(
        "\n4. **Why Groups? (Deterministic Transport Codes):**
   Codes like MPACT, SHIFT, or Serpent cannot track every energy point
   (millions of points across 1 eV–20 MeV range). Instead, they collapse
   the pointwise xs into ~30–1000 **energy groups**:

   σ_g = ∫ₑ_g σ(E, T) w(E) dE / ∫ₑ_g w(E) dE

   where w(E) is a weighting function (often a flux guess, e.g., 1/E).
   Each group has a single σ value (the group cross section), making
   the transport equation discretizable on modern computers."
    );

    let line = "═".repeat(70);
    println!("\n{}", line);
    println!("Summary");
    println!("{}", line);
    let ri_0k = resonance_integrals[0].1;
    let ri_room = resonance_integrals[1].1;
    let ri_hot = resonance_integrals[4].1;
    let pct_change = ((ri_hot - ri_0k) / ri_0k) * 100.0;

    println!("\nKey Results:");
    println!("  - Resonance integral at 0 K:        {:.4} barns", ri_0k);
    println!("  - Resonance integral at 293.6 K:    {:.4} barns", ri_room);
    println!("  - Resonance integral at 1200 K:     {:.4} barns", ri_hot);
    println!("  - Change from 0 K to 1200 K:        {:.1}%", pct_change);

    println!(
        "\nPublished reference value (ENDF/B-VIII.0):");
    println!(
        "  U-238 capture RI at thermal flux:   ~275 barns
   (This is at a specific dilution and flux-weighting; our value depends
    on integration bounds and our own weighting, so a quantitative match
    is not expected, but the temperature trend should be qualitatively correct.)"
    );

    let line = "═".repeat(70);
    println!("\n{}", line);
    println!("End of Tutorial");
    println!("{}", line);

    Ok(())
}
