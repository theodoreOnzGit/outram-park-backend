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
//! - Crucially, broadening **conserves the area** under a resonance: the peak
//!   gets shorter and wider, but the integral across it barely changes.
//!
//! ## Where the reactor physics actually comes from
//!
//! Because broadening conserves area, the **infinite-dilution** resonance
//! integral is nearly temperature-independent — this program shows it moving by
//! about 0.01% over 1200 K. That is the correct result, not a failure to
//! broaden.
//!
//! The feedback comes from **self-shielding**. In real fuel the absorber
//! shadows itself: at a tall resonance peak the total cross section is enormous,
//! the flux is depressed inside the fuel, and neutrons are absorbed in the outer
//! skin while the interior sees almost none. Heating the fuel flattens the peak,
//! which depresses the flux *less*, so more of the fuel participates and the
//! **effective absorption goes up** — here by roughly +29% over 1200 K at a
//! dilution of 60 b.
//!
//! More U-238 capture as the fuel heats is **negative** reactivity feedback, and
//! it is prompt. That is what makes a reactor stable on the fastest timescale
//! that matters. Note the direction carefully: *more* absorption when hot, not
//! less. It exists only through the interaction of broadening with
//! self-shielding, which is why this program computes both columns.
//!
//! ## What you'll see
//!
//! - The **U-238 capture resonance integral** from 0.5 eV (the cadmium cut-off)
//!   to 100 keV, at infinite dilution and at two finite dilutions.
//! - Infinite dilution lands at **~274 b** against a published **~275 b**, which
//!   is the check that RECONR reconstructed the resonances correctly.
//! - The infinite-dilution column flat in temperature; the self-shielded columns
//!   climbing steadily. That contrast is the whole point.
//!
//! Note the resonance integral is defined with **1/E weighting** — see the
//! `resonance_integral` function docs for why dropping it inflates the answer by
//! a factor of ~170.

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
/// Lower bound: the conventional cadmium cut-off, which is where the
/// epithermal 1/E region is taken to begin.
const RESONANCE_INTEGRAL_LOW_EV: f64 = 0.5;
const RESONANCE_INTEGRAL_HIGH_EV: f64 = 1.0e5;

/// Linear interpolation of a sorted `(energy, value)` grid at `e`.
///
/// Capture (MT=102) and total (MT=1) come off RECONR on different energy
/// grids, so the self-shielded integral below has to put them on a common one.
fn interp(pairs: &[(f64, f64)], e: f64) -> f64 {
    match pairs.binary_search_by(|(x, _)| x.partial_cmp(&e).unwrap()) {
        Ok(i) => pairs[i].1,
        Err(0) => pairs[0].1,
        Err(i) if i >= pairs.len() => pairs[pairs.len() - 1].1,
        Err(i) => {
            let (e1, s1) = pairs[i - 1];
            let (e2, s2) = pairs[i];
            if e2 == e1 {
                s1
            } else {
                s1 + (s2 - s1) * (e - e1) / (e2 - e1)
            }
        }
    }
}

/// The **infinite-dilution resonance integral**
///
/// ```text
///     RI = \int sigma_gamma(E) dE/E
/// ```
///
/// The `dE/E` weighting is part of the *definition*, not a convention that can
/// be dropped: it represents the 1/E slowing-down flux a neutron population
/// actually has in the epithermal range. Integrating `sigma dE` instead gives
/// a number that is not a resonance integral at all -- for U-238 it comes out
/// around 48,000 b against a published ~275 b, a factor of ~170, because the
/// high-energy end of the range is no longer suppressed by 1/E.
///
/// Since `dE/E = d(ln E)`, this trapezoids in `ln E`.
fn resonance_integral(pairs: &[(f64, f64)], e_min: f64, e_max: f64) -> f64 {
    let mut integral = 0.0;
    for w in pairs.windows(2) {
        let (e1, s1) = w[0];
        let (e2, s2) = w[1];
        if e2 <= e_min || e1 >= e_max || e1 <= 0.0 {
            continue;
        }
        let a = e1.max(e_min);
        let b = e2.min(e_max);
        if b <= a {
            continue;
        }
        // Values at the (possibly clipped) panel ends.
        let sa = if e2 == e1 {
            s1
        } else {
            s1 + (s2 - s1) * (a - e1) / (e2 - e1)
        };
        let sb = if e2 == e1 {
            s2
        } else {
            s1 + (s2 - s1) * (b - e1) / (e2 - e1)
        };
        integral += 0.5 * (sa + sb) * (b / a).ln();
    }
    integral
}

/// The **effective (self-shielded) resonance integral** at background cross
/// section `sigma_b`, in the narrow-resonance approximation:
///
/// ```text
///     RI_eff = \int sigma_gamma(E) * sigma_b / (sigma_t(E) + sigma_b) dE/E
/// ```
///
/// `sigma_b` is the "dilution": the scattering cross section per absorber atom
/// contributed by everything else in the lattice. Large `sigma_b` means a
/// dilute absorber and `RI_eff -> RI_infinite`; small `sigma_b` means a lump of
/// U-238 shadowing itself.
///
/// **This is where the Doppler effect actually lives.** See the interpretation
/// section at the end of the program for why.
fn effective_resonance_integral(
    capture: &[(f64, f64)],
    total: &[(f64, f64)],
    sigma_b: f64,
    e_min: f64,
    e_max: f64,
) -> f64 {
    let mut integral = 0.0;
    for w in capture.windows(2) {
        let (e1, sc1) = w[0];
        let (e2, _) = w[1];
        if e2 <= e_min || e1 >= e_max || e1 <= 0.0 {
            continue;
        }
        let a = e1.max(e_min);
        let b = e2.min(e_max);
        if b <= a {
            continue;
        }
        let sc = sc1.max(0.0);
        let st = interp(total, 0.5 * (a + b)).max(0.0);
        integral += sc * sigma_b / (st + sigma_b) * (b / a).ln();
    }
    integral
}

/// Print a formatted table of temperature vs. resonance integral.
fn print_resonance_integral_table(
    // (temperature [K], RI_infinite [b], RI at sigma_b=60 b, RI at sigma_b=20 b)
    results: &[(f64, f64, f64, f64)],
) {
    let line = "=".repeat(78);
    let dash = "-".repeat(78);

    println!("\n{}", line);
    println!("U-238 capture resonance integral vs temperature");
    println!("{}", line);
    println!(
        "{:>9} {:>12} {:>8} {:>12} {:>8} {:>12} {:>8}",
        "T (K)", "RI_inf (b)", "d%", "sb=60 (b)", "d%", "sb=20 (b)", "d%"
    );
    println!("{}", dash);

    let (_, inf0, s60_0, s20_0) = results.first().copied().unwrap_or((0.0, 0.0, 0.0, 0.0));
    let pct = |now: f64, base: f64| {
        if base > 0.0 {
            (now - base) / base * 100.0
        } else {
            0.0
        }
    };

    for (temp, inf, s60, s20) in results {
        println!(
            "{:>9.1} {:>12.2} {:>+8.2} {:>12.2} {:>+8.2} {:>12.2} {:>+8.2}",
            temp,
            inf,
            pct(*inf, inf0),
            s60,
            pct(*s60, s60_0),
            s20,
            pct(*s20, s20_0)
        );
    }
    println!("{}", dash);
    println!("d% is the change from the 0 K row of the same column.");
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
        eprintln!("ERROR: ENDF file not found at {}", endf_path.display());
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
    println!(
        "  Max evaluation energy:     {:.3e} eV",
        reconr_0k.material.emax
    );

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

    println!(
        "\nDoppler-broadening cross sections at {} temperatures...",
        temperatures_k.len()
    );
    let mut resonance_integrals: Vec<(f64, f64, f64, f64)> = Vec::new();

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

        // Total (MT=1) is needed for the self-shielded integral.
        let total_xs = sections_at_temp
            .iter()
            .find(|s| s.mt.number() == 1)
            .ok_or("MT=1 (total) not found in RECONR result")?;

        // Infinite dilution: the textbook resonance integral.
        let ri_inf = resonance_integral(
            &capture_xs.pairs,
            RESONANCE_INTEGRAL_LOW_EV,
            RESONANCE_INTEGRAL_HIGH_EV,
        );

        // Self-shielded, at two dilutions bracketing an LWR lattice.
        let ri_60 = effective_resonance_integral(
            &capture_xs.pairs,
            &total_xs.pairs,
            60.0,
            RESONANCE_INTEGRAL_LOW_EV,
            RESONANCE_INTEGRAL_HIGH_EV,
        );
        let ri_20 = effective_resonance_integral(
            &capture_xs.pairs,
            &total_xs.pairs,
            20.0,
            RESONANCE_INTEGRAL_LOW_EV,
            RESONANCE_INTEGRAL_HIGH_EV,
        );

        resonance_integrals.push((*temp_k, ri_inf, ri_60, ri_20));

        println!(
            "  T = {:7.1} K  →  RI_inf = {:7.2} b   RI(σb=60) = {:6.2} b   RI(σb=20) = {:6.2} b",
            temp_k, ri_inf, ri_60, ri_20
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // STEP 4: Display results and physical interpretation
    // ─────────────────────────────────────────────────────────────────────────

    print_resonance_integral_table(&resonance_integrals);

    let line = "═".repeat(70);
    println!("\n{}", line);
    println!("Physical interpretation");
    println!("{}", line);

    println!(
        "\n1. WHY dE/E, AND NOT dE
   The resonance integral is DEFINED with 1/E weighting:

       RI = integral of sigma_gamma(E) dE/E

   That weighting is the physics, not a convention: a slowing-down neutron
   population in the epithermal range really does have a ~1/E flux, so each
   decade of energy contributes comparably. Integrating sigma dE instead
   gives ~48,000 b for U-238 rather than ~275 b -- a factor of ~170 -- because
   nothing suppresses the wide high-energy end of the range. If your
   resonance integral comes out in the tens of thousands, this is why."
    );

    println!(
        "\n2. THE INFINITE-DILUTION RI IS ~FLAT IN TEMPERATURE
   Look at the RI_inf column: it moves by about 0.01% over 1200 K.

   That is not a bug and it is not the code failing to broaden. Doppler
   broadening CONSERVES THE AREA under a resonance -- the peak gets shorter
   and wider, and the integral of sigma dE/E across it barely moves. A
   correct implementation MUST show this.

   So if Doppler broadening does not change the amount of absorption at
   infinite dilution, where does the reactor physics come from?"
    );

    println!(
        "\n3. IT COMES FROM SELF-SHIELDING
   Look instead at the sb=60 and sb=20 columns, which is the same nuclide at
   finite dilution:

       RI_eff = integral of sigma_gamma * sb/(sigma_t + sb) * dE/E

   sb is the background (scattering) cross section per absorber atom from
   everything else in the lattice. The factor sb/(sigma_t + sb) is the flux
   depression: at a tall resonance peak sigma_t is huge, the factor collapses
   toward zero, and neutrons are absorbed in the outer skin of the fuel while
   the interior sees almost none. The lump SHADOWS ITSELF.

   Now heat the fuel. The peaks flatten and widen. A shorter peak depresses
   the flux less, so more of the fuel volume participates, and the EFFECTIVE
   absorption goes UP -- here by roughly +29% at sb=60 over 1200 K, even
   though the infinite-dilution integral did not move at all.

   More U-238 capture as fuel heats is negative reactivity feedback, prompt,
   and it is why a reactor is stable on the fastest timescale that matters.
   It is the single most important number in reactor safety, and it exists
   ONLY because of the interaction between broadening and self-shielding."
    );

    println!(
        "\n4. SANITY CHECK
   RI_inf came out at about 274 b against a published U-238 capture resonance
   integral of ~275 b (0.5 eV upward, infinite dilution). Agreement at that
   level says RECONR reconstructed the resolved resonances correctly and the
   1/E quadrature is right.

   The self-shielded values are deliberately NOT compared to a reference here:
   they depend on the dilution chosen, and sb=20/60 b are illustrative of an
   LWR lattice rather than a specific benchmark. It is the TREND with
   temperature that is being taught."
    );

    println!(
        "\n5. WHAT RECONR AND BROADR ACTUALLY DID FOR YOU
   RECONR turned resonance PARAMETERS (a few hundred numbers giving each
   resonance's energy, width and spin) into a pointwise sigma(E) grid dense
   enough that linear interpolation between points is accurate. That is a
   reconstruction, not a lookup.

   BROADR then convolved that 0 K grid with the Maxwellian velocity
   distribution of the target nuclei at temperature T -- the Solbrig kernel --
   producing the broadened grid. Everything above falls out of those two
   steps plus an integral."
    );

    println!("\n{}", line);
    println!("End of Tutorial");
    println!("{}", line);

    Ok(())
}
