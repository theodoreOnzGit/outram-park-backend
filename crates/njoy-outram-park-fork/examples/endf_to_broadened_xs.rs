//! # ENDF → Pointwise → Doppler-Broadened Cross Sections
//!
//! This tutorial example demonstrates the core nuclear-data processing workflow:
//! 1. Read an ENDF evaluated nuclear data file from disk
//! 2. Reconstruct pointwise cross sections (RECONR)
//! 3. Apply Doppler broadening to simulate the effect of temperature (BROADR)
//! 4. Display the results in a table
//!
//! ## What is Doppler Broadening?
//!
//! At 0 K, nuclei are stationary and cross sections have sharp resonances.
//! At finite temperature, nuclei move randomly (thermal motion), which shifts the
//! Doppler effect — the apparent energy of an incoming neutron. This causes:
//! - Resonance peaks to broaden (wider, lower amplitude)
//! - Dips between resonances to fill in partially
//!
//! The free-gas model used here assumes the nucleus recoils freely. The
//! SIGMA1 algorithm integrates the collision kernel analytically over energy
//! panels, making it fast and accurate.
//!
//! ## Physical Interpretation
//!
//! U-238 has strong resolved resonances in the keV range. When you compare
//! 293.6 K (room temperature) and 900 K (elevated), the narrower, deeper
//! resonances at 900 K reflect the higher thermal motion blurring out the peaks.
//!
//! ## Usage
//!
//! ```bash
//! # Use default U-238 reference file
//! cargo run --release --example endf_to_broadened_xs
//!
//! # Or specify your own ENDF file
//! cargo run --release --example endf_to_broadened_xs -- /path/to/evaluation.endf
//! ```

// Everything this example needs comes from the prelude — that is the
// intended entry point, and it is worth checking that it suffices.
use njoy_outram_park_fork::prelude::*;
use std::path::PathBuf;

/// Return the path to the default reference U-238 ENDF file.
/// Resolves relative to CARGO_MANIFEST_DIR so it works from any cwd.
fn default_endf_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../reference-data/endf/n-092_U_238.endf");
    p
}

/// Print a table of energy vs cross section, with columns for energy [eV],
/// cross section at T1 [barns], and cross section at T2 [barns].
/// Shows only a subset of points to keep output readable.
fn print_comparison_table(
    title: &str,
    energies: &[(f64, f64, f64)], // (energy, xs_t1, xs_t2)
    temp1_k: u32,
    temp2_k: u32,
) {
    println!("\n{}", title);
    println!("{}", "═".repeat(80));
    println!(
        "{:>15} {:>20} {:>20}",
        "Energy (eV)",
        format!("σ @ {} K (b)", temp1_k),
        format!("σ @ {} K (b)", temp2_k),
    );
    println!("{}", "─".repeat(80));

    // Show points evenly spaced in log-energy space for clarity
    let n_display = 20;
    let step = (energies.len() - 1).max(1) / (n_display - 1).max(1);
    for (i, (e, xs_t1, xs_t2)) in energies.iter().enumerate() {
        if i % step == 0 || i == energies.len() - 1 {
            println!("{:15.6e} {:20.6e} {:20.6e}", e, xs_t1, xs_t2);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ────────────────────────────────────────────────────────────────────────────
    // STEP 0: Determine which ENDF file to use
    // ────────────────────────────────────────────────────────────────────────────

    let endf_path = if let Some(arg) = std::env::args().nth(1) {
        PathBuf::from(arg)
    } else {
        default_endf_path()
    };

    println!("Reading ENDF file: {}", endf_path.display());
    if !endf_path.exists() {
        eprintln!("Error: ENDF file not found at {}", endf_path.display());
        std::process::exit(1);
    }

    // ────────────────────────────────────────────────────────────────────────────
    // STEP 1: Read the ENDF evaluation
    // ────────────────────────────────────────────────────────────────────────────
    //
    // An ENDF tape is a structured file format (ENDF-6) containing:
    // - MF=1: General information (material header, resonance flags, max energy)
    // - MF=2: Resonance parameters (if any)
    // - MF=3: Cross sections (smooth background)
    // - MF=7+: Thermal scattering, angular distributions, etc.
    //
    // Tape::read_file parses the whole file into an in-memory structure.
    // (Tape::read is the generic form, for a socket or a decompressor; for a
    // file on disk this is the one you want.)

    let tape = Tape::read_file(&endf_path)?;

    println!("✓ Read ENDF tape with {} sections", tape.sections().len());

    // ────────────────────────────────────────────────────────────────────────────
    // STEP 2: Reconstruct pointwise cross sections (RECONR)
    // ────────────────────────────────────────────────────────────────────────────
    //
    // RECONR converts the compact ENDF representation into a fine pointwise grid:
    // - Takes MF=2 resonance parameters and MF=3 cross sections
    // - Generates a lin-lin interpolated (energy, sigma) grid
    // - Resonance peaks are resolved and added to the smooth background
    // - Temperature is 0 K here; Doppler shift happens in BROADR (STEP 4)
    //
    // The output is a ReconrResult with:
    // - material: ZA, AWR (atomic weight ratio), etc.
    // - sections: Vec of ReconrSection, one per MF=3 reaction (MT number)

    // Auto-detect the material number from the ENDF file (most files have one)
    let materials = tape.materials();
    let mat = *materials.first().ok_or("No materials found in ENDF file")?;

    let config = ReconrConfig {
        mat,              // Use the material number from the file
        tolerance: 0.001, // 0.1% linearization error — typical NJOY default
        temperature: 0.0, // 0 K: no Doppler shift at reconstruction
    };

    let reconr_result = reconr(&tape, &config)?;
    println!(
        "✓ Reconstructed {} reactions at 0 K (MAT {}, AWR = {:.4})",
        reconr_result.sections.len(),
        config.mat,
        reconr_result.material.awr
    );

    // ────────────────────────────────────────────────────────────────────────────
    // STEP 3: Reconstruct at another temperature to show the effect
    // ────────────────────────────────────────────────────────────────────────────
    //
    // If we run RECONR again at a different temperature, the internal SLBW/
    // Reich-Moore evaluation shifts the resonances (Doppler pre-shift), but for
    // a cleaner tutorial, we use BROADR instead (next step), which broadens the
    // 0 K grid in velocity space. Either way, you get the same physics; BROADR
    // is faster for multiple temperatures.

    // ────────────────────────────────────────────────────────────────────────────
    // STEP 4: Apply Doppler broadening to generate cross sections at multiple T
    // ────────────────────────────────────────────────────────────────────────────
    //
    // BROADR takes:
    // - sections: the 0 K reconstructed pointwise grid (from RECONR)
    // - awr: atomic weight ratio (mass of nucleus / mass of neutron)
    // - temp_k: target temperature [K]
    //
    // and returns a new set of ReconrSection with the same energy grid but
    // broadened cross sections (convolved with the free-gas kernel).
    //
    // The algorithm:
    // 1. Transform the energy grid into velocity space: u = √(α·E)
    //    where α = AWR / (k_B · T)
    // 2. For each output energy, integrate the collision kernel over the
    //    input cross-section grid using the SIGMA1 method (analytic f-functions)
    // 3. Return the same energy grid (in eV) but with broadened sigma values

    let temp1_k = 293.6; // Room temperature
    let temp2_k = 900.0; // Elevated temperature (hotter reactor)

    let broadened_293k =
        doppler_broaden(&reconr_result.sections, reconr_result.material.awr, temp1_k);
    let broadened_900k =
        doppler_broaden(&reconr_result.sections, reconr_result.material.awr, temp2_k);

    println!(
        "✓ Broadened cross sections to {} K and {} K",
        temp1_k as u32, temp2_k as u32
    );

    // ────────────────────────────────────────────────────────────────────────────
    // STEP 5: Display results for the total cross section (MT=1)
    // ────────────────────────────────────────────────────────────────────────────
    //
    // MT (ENDF reaction type) numbers:
    // - MT=1: total cross section (elastic + inelastic + absorption + ...)
    // - MT=2: elastic scattering
    // - MT=102: radiative capture (n,γ)
    // - MT=18: fission
    // - See ENDF-6 format specification for the full list
    //
    // The total cross section is the easiest to visualize because it combines
    // all the resonance structure.

    // Find the total cross section (MT=1)
    let total_xs_293k = broadened_293k
        .iter()
        .find(|s| s.mt.number() == 1)
        .ok_or("MT=1 (total) not found in broadened result")?;

    let total_xs_900k = broadened_900k
        .iter()
        .find(|s| s.mt.number() == 1)
        .ok_or("MT=1 (total) not found in broadened result")?;

    // Collect energy-vs-sigma triples for display
    let mut comparison: Vec<(f64, f64, f64)> = total_xs_293k
        .pairs
        .iter()
        .zip(total_xs_900k.pairs.iter())
        .map(|((e1, s1), (e2, s2))| {
            // Sanity check: both grids should have the same energy points
            assert!(
                (e1 - e2).abs() < 1e-9 * e1.max(1.0),
                "Energy mismatch in broadened grids"
            );
            (*e1, *s1, *s2)
        })
        .collect();

    // Sort by energy (should already be sorted, but let's be safe)
    comparison.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // Print the table
    print_comparison_table(
        "Total Cross Section (MT=1) at Two Temperatures",
        &comparison,
        temp1_k as u32,
        temp2_k as u32,
    );

    // ────────────────────────────────────────────────────────────────────────────
    // STEP 6: Summary and analysis
    // ────────────────────────────────────────────────────────────────────────────

    println!("\n{}", "═".repeat(80));
    println!("Summary");
    println!("{}", "═".repeat(80));
    println!("Material ZA:              {}", reconr_result.material.za);
    println!(
        "Atomic weight ratio (AWR): {:.6}",
        reconr_result.material.awr
    );
    println!(
        "Max evaluation energy:      {:.3e} eV",
        reconr_result.material.emax
    );
    println!(
        "Reconstructed reactions:   {}",
        reconr_result.sections.len()
    );
    println!("Total XS grid points:       {}", total_xs_293k.pairs.len());
    println!(
        "Energy range:               {:.3e} – {:.3e} eV",
        comparison[0].0,
        comparison[comparison.len() - 1].0
    );

    println!("\nObservations:");
    println!("- At {:.1} K: resonances are narrower and sharper", temp1_k);
    println!(
        "- At {:.1} K: resonances are broader and lower (Doppler smearing)",
        temp2_k
    );
    println!("- Deep dips between peaks partially fill in at higher T");
    println!("- This accounts for neutron moderation and absorption in thermal systems");

    Ok(())
}
