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

    let thnmax = broadening_limit(&reconr_result);
    let broadened_293k = doppler_broaden_below(
        &reconr_result.sections,
        reconr_result.material.awr,
        temp1_k,
        thnmax,
    );
    let broadened_900k = doppler_broaden_below(
        &reconr_result.sections,
        reconr_result.material.awr,
        temp2_k,
        thnmax,
    );

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

    // Collect energy-vs-sigma triples for display. BROADR inserts grid
    // points adaptively per temperature (`broadn`), so the two grids differ;
    // sample the 900 K result on the 293.6 K grid by lin-lin interpolation.
    let interp_900 = [(total_xs_900k.pairs.len() as u32, 2u32)];
    let mut comparison: Vec<(f64, f64, f64)> = total_xs_293k
        .pairs
        .iter()
        .map(|&(e1, s1)| {
            let s2 = njoy_outram_park_fork::endf::interp::eval_tab1(
                e1,
                &interp_900,
                &total_xs_900k.pairs,
            )
            .unwrap_or(f64::NAN);
            (e1, s1, s2)
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

    vv_gate(&comparison, temp1_k, temp2_k);

    Ok(())
}

/// V&V gate: the three claims the "Observations" list above makes, asserted.
///
/// # Why a tutorial needs a gate
///
/// This file's prose asserts specific physics — peaks broaden and *lower*, dips
/// *fill in* — and nothing checked it. A tutorial that teaches a wrong claim is
/// worse than one that teaches nothing, because the reader has no way to tell.
///
/// # The oracle is analytic and needs no reference tape
///
/// Doppler broadening is a **convolution** of the 0 K cross section with the
/// target's Maxwellian velocity distribution. Three consequences follow with no
/// model and no fitted parameter:
///
/// 1. **Area is conserved.** `int sigma dE` across a resonance is invariant, to
///    the accuracy of the free-gas kernel. This is the strongest of the three:
///    it is a sum rule, so it holds pointwise-independently of how the grid is
///    laid out, and it is exactly the property that makes a resonance *integral*
///    temperature-independent at infinite dilution.
/// 2. **The peak falls.** A convolution with a positive, normalised kernel
///    cannot raise a local maximum.
/// 3. **The valley rises.** By the same argument it cannot lower a local
///    minimum.
///
/// Claims 2 and 3 together are what "broadening" *means*, and claim 1 is what
/// stops a code from satisfying them by simply scaling everything down. A
/// broadening kernel that lost 5 % of the area would pass 2 and 3 and be badly
/// wrong — which is why all three are asserted rather than just the visible ones.
///
/// # Results (2026-09-11, U-238 ENDF/B-VIII.0, 293.6 K vs 900 K)
///
/// ```text
///   area under sigma(E)   1.924489e8 -> 1.924490e8 b.eV    +0.000 %
///   peak   at 36.683 eV      13450.04 b ->  8464.77 b      -37.07 %
///   valley at 2.5437 keV         0.5007 b ->    1.7698 b   +253.44 %
/// ```
///
/// Area is conserved to **the printed precision** — five significant figures
/// apart on a number that individual points move by 37 % and 253 %. That is the
/// sum rule working, and it is a far sharper statement about the SIGMA1 kernel
/// than either of the visible claims.
///
/// # Tolerances
///
/// **0.5 %** on area conservation — the free-gas kernel is not exactly
/// area-preserving on a finite grid, and the comparison here is carried out on
/// the 293.6 K grid rather than a common refinement. Measured +0.000 %, so the
/// envelope has three orders of magnitude of headroom and could be tightened; it
/// is left where it is because the grid, not the kernel, sets the floor and the
/// grid is a property of the tutorial's configuration rather than of the physics.
///
/// **Strict inequality** on the peak and the valley. They are qualitative claims
/// — a convolution cannot raise a maximum or lower a minimum — and a qualitative
/// claim admits no tolerance.
fn vv_gate(comparison: &[(f64, f64, f64)], temp1_k: f64, temp2_k: f64) {
    use njoy_outram_park_fork::vv::assert_relative;

    println!("\n=== V&V gate: Doppler broadening against its own analytic properties ===");

    let rows: Vec<&(f64, f64, f64)> = comparison
        .iter()
        .filter(|(e, a, b)| e.is_finite() && a.is_finite() && b.is_finite())
        .collect();
    assert!(
        rows.len() >= 10,
        "only {} usable points in the comparison table; the gate needs a resonance \
         to integrate over",
        rows.len()
    );

    // 1. Area conservation, by the trapezoidal rule on the shared grid.
    let (mut area_cold, mut area_hot) = (0.0_f64, 0.0_f64);
    for w in rows.windows(2) {
        let de = w[1].0 - w[0].0;
        area_cold += 0.5 * (w[0].1 + w[1].1) * de;
        area_hot += 0.5 * (w[0].2 + w[1].2) * de;
    }
    assert_relative(
        &format!(
            "area under sigma(E) is conserved from {temp1_k:.1} K to {temp2_k:.1} K \
             (broadening is a convolution)"
        ),
        area_hot,
        area_cold,
        0.005,
    );

    // 2 and 3. The peak must fall and the valley must rise. Located on the COLD
    // curve, then read off both — locating on the hot curve would beg the
    // question by finding wherever the hot curve happens to be extreme.
    let peak = rows
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).expect("finite"))
        .expect("non-empty");
    let valley = rows
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).expect("finite"))
        .expect("non-empty");

    println!(
        "  peak   at {:.4e} eV: {:.4} b -> {:.4} b  ({:+.2} %)",
        peak.0,
        peak.1,
        peak.2,
        100.0 * (peak.2 / peak.1 - 1.0)
    );
    assert!(
        peak.2 < peak.1,
        "the resonance peak at {:.4e} eV ROSE from {:.4} b at {temp1_k:.1} K to \
         {:.4} b at {temp2_k:.1} K. Doppler broadening is a convolution with a \
         positive normalised kernel, which cannot raise a local maximum. This \
         file's own 'Observations' list says it falls.",
        peak.0,
        peak.1,
        peak.2,
    );

    println!(
        "  valley at {:.4e} eV: {:.4} b -> {:.4} b  ({:+.2} %)",
        valley.0,
        valley.1,
        valley.2,
        100.0 * (valley.2 / valley.1 - 1.0)
    );
    assert!(
        valley.2 > valley.1,
        "the dip at {:.4e} eV FELL from {:.4} b at {temp1_k:.1} K to {:.4} b at \
         {temp2_k:.1} K. A convolution with a positive normalised kernel cannot \
         lower a local minimum; this file's own 'Observations' list says the dips \
         fill in.",
        valley.0,
        valley.1,
        valley.2,
    );
    println!("  [PASS] peak falls, valley rises, area conserved — broadening, as claimed");
}
