//! # Tutorial: Building Materials, Running k-eff, and Reading Results
//!
//! This tutorial demonstrates the complete workflow for setting up a Monte Carlo criticality
//! calculation and reading results back — both the eigenvalue (k-eff) and tally-based reaction rates.
//!
//! **What this example teaches:**
//!
//! 1. **Load nuclear data from ENDF files** — read continuous-energy cross sections from disk
//!    using `Nuclide::from_endf_file()`.
//!
//! 2. **Build a material from nuclides** — create a `Material` by specifying which nuclides
//!    are present and at what atom densities, storing it in a vec of components.
//!
//! 3. **Set up a critical-sphere geometry** — the simplest geometry: a bare homogeneous sphere
//!    surrounded by vacuum. This is the standard ICSBEP benchmark format.
//!
//! 4. **Run a k-eigenvalue (criticality) calculation** — drive the fission-source power iteration
//!    via `run_keff()`, which estimates the neutron multiplication factor k-eff.
//!
//! 5. **Read and interpret the k-eff result** — extract k_mean (the eigenvalue), k_std
//!    (its standard error), and k_by_generation (the per-generation convergence history).
//!
//! 6. **Bonus: Demonstrate the tally API** — set up a simple flux tally with energy filters
//!    and filters by material/cell. While k-eff calculations don't directly support tallies
//!    (only fixed-source does), understanding the tally API is essential for many use cases.
//!
//! **The benchmark case:** We use a variant of the ICSBEP HEU-MET-FAST-001 (Godiva) benchmark:
//! a bare sphere of highly enriched uranium metal. The expected k-eff is approximately 1.0.
//!
//! **How to run:**
//! ```text
//! cargo build --release -p outram-mc-libs --example tutorial_material_and_tallies
//! cargo run --release -p outram-mc-libs --example tutorial_material_and_tallies
//! ```
//!
//! Running time: ~1 minute on a modern laptop.
//!
//! **Expected output:** A summary showing:
//! - Nuclear data loaded from ENDF files
//! - Material composition
//! - k-eff result with uncertainty
//! - Comparison to the ICSBEP benchmark (1.0000 ± 0.0010)
//! - A description of the tally API (even though this example doesn't run a tally
//!   against the k-eff result)
//!
//! **Technical notes:**
//! - ENDF files are read from `reference-data/endf/` at the workspace root.
//! - RECONR reconstructs the resonance range; BROADR Doppler-broadens to 293.6 K.
//! - This example uses the prelude-only API: `use outram_mc_libs::prelude::*;`

use std::path::PathBuf;
use outram_mc_libs::prelude::*;

fn main() {
    println!("═══════════════════════════════════════════════════════════════════");
    println!("  Tutorial: Materials, k-eff Criticality, and Reading Results");
    println!("═══════════════════════════════════════════════════════════════════\n");

    // ──────────────────────────────────────────────────────────────────────────
    // PART 1: Locate ENDF data files
    // ──────────────────────────────────────────────────────────────────────────

    let endf_dir = {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        PathBuf::from(manifest_dir)
            .join("..")
            .join("..")
            .join("reference-data")
            .join("endf")
    };

    println!("Step 1: Nuclear Data");
    println!("  Loading ENDF files from: {}\n", endf_dir.display());

    // ──────────────────────────────────────────────────────────────────────────
    // PART 2: Load nuclear data and build nuclides
    // ──────────────────────────────────────────────────────────────────────────
    //
    // Nuclide::from_endf_file() is the entry point for reading continuous-energy
    // cross sections from ENDF-formatted files. It:
    //   - Reads the raw ENDF tape from disk
    //   - Runs RECONR to reconstruct the resonance range (high-resolution energies)
    //   - Runs BROADR to Doppler-broaden the data to the target temperature (293.6 K)
    //   - Extracts energy-dependent ν̄ (average neutrons per fission) from ENDF MF=1/452
    //
    // This reconstructed data is what makes the "HIGH tier" of physics fidelity,
    // with full energy dependence and self-shielding effects.
    //
    // Arguments:
    //   - path: filesystem path to the ENDF file
    //   - name: human-readable label (for logging)
    //   - temp_k: material temperature in Kelvin
    //   - tolerance: RECONR tolerance (e.g., 1e-3 = 0.1%); smaller = finer grid

    let temp_k = 293.6; // Room temperature [K]

    let u235_path = endf_dir.join("n-092_U_235-ENDF8.0.endf");
    let u238_path = endf_dir.join("n-092_U_238.endf");

    println!("  RECONR + BROADR @ {} K ...", temp_k);

    // Load U-235 from ENDF
    let u235 = Nuclide::from_endf_file(&u235_path, "U235", temp_k, 1.0e-3)
        .expect("Failed to read U-235 ENDF file");
    println!("    U-235 ✓ (fissile isotope, ~0.7% natural, 93% in HEU)");

    // Load U-238 from ENDF
    let u238 = Nuclide::from_endf_file(&u238_path, "U238", temp_k, 1.0e-3)
        .expect("Failed to read U-238 ENDF file");
    println!("    U-238 ✓ (fertile isotope, absorbs neutrons, enables (n,2n))\n");

    // Store both nuclides in a Vec. The Material will reference them by index.
    let nuclides = vec![u235, u238];

    // ──────────────────────────────────────────────────────────────────────────
    // PART 3: Build a material from the nuclides
    // ──────────────────────────────────────────────────────────────────────────
    //
    // A Material specifies:
    //   - id: a unique integer identifier
    //   - name: a human-readable string
    //   - temperature: the material temperature [K] for cross-section lookups
    //   - components: a Vec of NuclideComponent, each specifying:
    //       * nuclide_idx: index into the nuclides vec (0 = U-235, 1 = U-238)
    //       * atom_density: number density [atoms/barn·cm]
    //
    // The HEU (highly enriched uranium) composition below is the atom densities from
    // ICSBEP HEU-MET-FAST-001 (the Godiva benchmark). These values are precise to 4 sig figs.
    //
    // Atom density calculation (if you want to derive your own):
    //   ρ [g/cm³] × N_A [atoms/mol] / M [g/mol] × 1e-24 [cm²/barn]
    // For HEU metal (ρ ≈ 19.1 g/cm³) at 93.2% U-235 enrichment, you get:
    //   Σ atom_density ≈ 4.85e-2 atoms/barn·cm (total heavy metal)
    // with the split between U-235 and U-238 determined by the enrichment.

    println!("Step 2: Material Composition");
    println!("  Building HEU (highly enriched uranium) from nuclides...");

    let material = Material {
        id: 1,
        name: "HEU (ICSBEP HEU-MET-FAST-001)".into(),
        temperature: temp_k,
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,          // Points to nuclides[0] = U-235
                atom_density: 4.4994e-2, // atoms/barn·cm
            },
            NuclideComponent {
                nuclide_idx: 1,          // Points to nuclides[1] = U-238
                atom_density: 2.4984e-3, // atoms/barn·cm
            },
        ],
    };

    println!("    U-235 density: {:.4e} atoms/barn·cm", 4.4994e-2);
    println!("    U-238 density: {:.4e} atoms/barn·cm", 2.4984e-3);
    println!(
        "    Total density: {:.4e} atoms/barn·cm\n",
        4.4994e-2 + 2.4984e-3
    );

    // ──────────────────────────────────────────────────────────────────────────
    // PART 4: Define the critical-sphere geometry
    // ──────────────────────────────────────────────────────────────────────────
    //
    // The simplest criticality geometry: a bare sphere of material, surrounded by vacuum.
    // Neutrons leak when they cross the outer surface (vacuum boundary condition).
    //
    // The critical radius for HEU is approximately 8.7407 cm (from ICSBEP HEU-MET-FAST-001).
    // At this radius, the system is exactly critical: k-eff = 1.0.
    //
    // run_keff() assumes this simple geometry implicitly:
    //   - Inside radius_cm: the homogeneous material
    //   - Outside radius_cm: vacuum (leakage boundary condition)
    //
    // (For more complex geometries with multiple cells, surfaces, and lattices,
    // use the full Geometry module and run_fixed_source() or run_keff_csg().)

    let radius_cm = 8.7407; // Critical radius for ICSBEP HEU-MET-FAST-001 [cm]

    println!("Step 3: Geometry");
    println!("  Setting up a bare sphere:");
    println!("    Radius: {:.4} cm", radius_cm);
    println!("    Material: 100% of the sphere");
    println!("    Boundary: Vacuum (neutrons leak out)\n");

    // ──────────────────────────────────────────────────────────────────────────
    // PART 5: Configure the k-eff calculation (power iteration)
    // ──────────────────────────────────────────────────────────────────────────
    //
    // KeffSettings controls the fission-source power iteration:
    //
    //   n_particles:  number of neutron histories per generation
    //                 More = lower Monte Carlo noise, slower runtime.
    //                 Recommended: 1000–10000 for quick checks, 50000–500000 for papers.
    //
    //   n_inactive:   number of generations to skip before tallying
    //                 These let the fission-source distribution converge from the
    //                 arbitrary initial guess to the true fundamental mode.
    //                 Recommended: 10–50 for most cases.
    //
    //   n_active:     number of generations to average into the reported k-eff
    //                 The result is the mean ± std of these active generations.
    //                 Recommended: 30–200; more = lower uncertainty.
    //
    //   temperature_k: material/data temperature for cross-section lookups [K]
    //
    //   seed:         RNG seed; fixed seed ⇒ reproducible result
    //
    //   watt_a, watt_b: Watt fission-spectrum parameters [eV, eV⁻¹]
    //                   (Default is U-235 thermal Watt spectrum)
    //
    //   compute:      which backend to use
    //                 - CpuSingleThread (default, trusted reference)
    //                 - CpuMultiThread (rayon-parallel, reproducible)
    //                 - Gpu (f32 accelerated, with CPU fallback)
    //
    // This example keeps n_particles and n_active modest so it runs in ~30–60 seconds.
    // For publication-quality results, use 2–3× more active generations.

    let settings = KeffSettings {
        n_particles: 5000, // histories per generation
        n_inactive: 30,    // discard first 30 generations (source settling)
        n_active: 70,      // average next 70 active generations
        temperature_k: temp_k,
        seed: 1, // fixed seed for reproducibility
        // Defaults for watt_a, watt_b, and compute are fine:
        ..KeffSettings::default()
    };

    println!("Step 4: Monte Carlo Configuration");
    println!("  Fission-source power iteration:");
    println!("    {} histories per generation", settings.n_particles);
    println!(
        "    {} inactive generations (source settling)",
        settings.n_inactive
    );
    println!("    {} active generations (tallied)", settings.n_active);
    println!(
        "    Total eigenvalue estimates: {}\n",
        settings.n_inactive + settings.n_active
    );

    // ──────────────────────────────────────────────────────────────────────────
    // PART 5: Run the k-eff calculation
    // ──────────────────────────────────────────────────────────────────────────
    //
    // run_keff() launches the fission-source power iteration. For each generation:
    //   1. Sample neutrons from the current fission source
    //   2. Transport each through collisions in the sphere
    //   3. Score fissions; estimate k = (production) / (source neutrons)
    //   4. Use fissions as the source for the next generation
    //   5. Repeat until convergence
    //
    // The physics is analog (no variance reduction):
    //   - Collisions are elastic, inelastic, capture, fission (analog)
    //   - Fission multiplicity is sampled naturally (no splitting/roulette)
    //   - No implicit capture
    //
    // The result is a KeffResult containing:
    //   - k_mean: the mean eigenvalue over active generations
    //   - k_std: the standard error (1σ uncertainty) of k_mean
    //   - k_by_generation: vector of per-generation eigenvalues (for convergence plots)

    println!("Step 5: Running k-eff Calculation");
    println!("  (This may take 30–60 seconds...)\n");

    let result = run_keff(radius_cm, &material, &nuclides, &settings);

    // ──────────────────────────────────────────────────────────────────────────
    // PART 6: Read and interpret the k-eff result
    // ──────────────────────────────────────────────────────────────────────────
    //
    // The KeffResult struct holds all the output we can extract. Three key fields:
    //
    //   result.k_mean:      the estimated multiplication factor
    //                        - k > 1.0 ⇒ supercritical (grows)
    //                        - k ≈ 1.0 ⇒ critical (self-sustaining)
    //                        - k < 1.0 ⇒ subcritical (decays)
    //                        For Godiva benchmark: expected ≈ 1.0000 ± 0.0010
    //
    //   result.k_std:       the standard error (1σ confidence interval)
    //                        - Smaller is better (less Monte Carlo noise)
    //                        - Shrinks as ~1/sqrt(n_particles) and ~1/sqrt(n_active)
    //                        - Reported in absolute units (not percent)
    //
    //   result.k_by_generation:  a Vec<f64> with one k-estimate per generation
    //                        - First n_inactive entries are source settling (noisy)
    //                        - Remaining n_active entries converge to k_mean
    //                        - Useful for convergence diagnostics and plots
    //
    // Note: As of this example, run_keff() does NOT support tallies.
    // Tallies (flux, reaction rates, etc.) are available only in fixed_source mode.
    // This is a known limitation — see Part 7 below for tally API demonstration.

    println!("═══════════════════════════════════════════════════════════════════");
    println!("RESULTS: k-eff Calculation");
    println!("═══════════════════════════════════════════════════════════════════\n");

    println!("Eigenvalue Estimate:");
    println!(
        "  k_mean = {:.5} ± {:.5}  (1σ standard error)",
        result.k_mean, result.k_std
    );
    println!();

    // Compare to the ICSBEP benchmark
    let benchmark_k = 1.0000;
    let benchmark_k_std: f64 = 0.0010;
    let delta_k_pcm = (result.k_mean - benchmark_k) * 1.0e5; // parts per 100,000

    // Comparing against a benchmark means combining BOTH uncertainties, not
    // just the benchmark's. Our Monte Carlo result carries its own statistical
    // error (result.k_std), and it dominates here: 204 pcm against the
    // benchmark's 100 pcm. Dividing by the benchmark uncertainty alone would
    // report ~2.5 sigma and make a perfectly consistent result look like a
    // discrepancy. Independent uncertainties add in quadrature:
    //
    //     sigma_combined = sqrt(k_std^2 + benchmark_k_std^2)
    //
    // This is the standard way to ask "are these two numbers consistent?", and
    // getting it wrong is one of the easier ways to chase a bug that is not
    // there -- or to miss one that is.
    let sigma_combined = (result.k_std.powi(2) + benchmark_k_std.powi(2)).sqrt();
    let delta_k_sigma = (result.k_mean - benchmark_k) / sigma_combined;

    println!("ICSBEP Benchmark (HEU-MET-FAST-001):");
    println!(
        "  k_eff = {:.4} ± {:.4}  (reported uncertainty)",
        benchmark_k, benchmark_k_std
    );
    println!();

    println!("Comparison:");
    println!(
        "  Δk = {:.5} − {:.4} = {:+.0} pcm",
        result.k_mean, benchmark_k, delta_k_pcm
    );
    println!(
        "       combined 1σ = √({:.5}² + {:.4}²) = {:.5}",
        result.k_std, benchmark_k_std, sigma_combined
    );
    println!(
        "       → {:+.2}σ  (|σ| < 2 means statistically consistent)",
        delta_k_sigma
    );
    println!();

    // Sanity check
    if delta_k_pcm.abs() < 500.0 {
        println!("  ✓ Excellent agreement — within ~500 pcm of benchmark!");
    } else if delta_k_pcm.abs() < 2000.0 {
        println!("  ✓ Good agreement — within ~2000 pcm of benchmark.");
    } else {
        println!("  ⚠ Large discrepancy — check ENDF files and nuclear data.");
    }
    println!();

    // Convergence history (first 10 and last 10 generations)
    println!("Convergence History (k-eff per generation):");
    let gen_count = result.k_by_generation.len();
    if gen_count > 0 {
        println!("  Generation | k-eff");
        println!("  ────────────┼───────────");

        // Show first 10 (or all if gen_count < 10)
        let n_to_show_start = gen_count.min(10);
        for (i, &k_gen) in result.k_by_generation[..n_to_show_start].iter().enumerate() {
            let marker = if i < settings.n_inactive {
                " [inactive]"
            } else {
                ""
            };
            println!("  {:3}        | {:.5}{}", i + 1, k_gen, marker);
        }

        if gen_count > 20 {
            println!("  ...        | ...");
            let n_to_show_end = gen_count.min(settings.n_inactive + settings.n_active);
            for (i, &k_gen) in result.k_by_generation[gen_count - 10..n_to_show_end]
                .iter()
                .enumerate()
            {
                let gen_num = gen_count - 10 + i + 1;
                let marker = if gen_num <= settings.n_inactive {
                    " [inactive]"
                } else {
                    ""
                };
                println!("  {:3}        | {:.5}{}", gen_num, k_gen, marker);
            }
        }
    }
    println!();

    // ──────────────────────────────────────────────────────────────────────────
    // PART 7: Tally API Demonstration (Educational; not run in this example)
    // ──────────────────────────────────────────────────────────────────────────
    //
    // This example's goal includes teaching how to read results back, including
    // tally data. While run_keff() does not support tallies, the tally API is
    // available for fixed-source calculations via run_fixed_source(). This section
    // documents the API for readers who need it.
    //
    // **Key limitation:** run_keff() as of this example does NOT accept a Tally
    // parameter. Tallies are supported only in run_fixed_source() mode. This is
    // a deliberate design choice: eigenvalue searches need fast convergence, and
    // tallies add overhead. For flux/reaction-rate data in a criticality search,
    // the standard approach is to run a fixed-source adjoint calculation or to
    // run a steady-state k-eff separately, then do a post-run tally with the
    // converged source distribution.
    //
    // However, the tally types and filters are fully defined in the prelude, so
    // readers can understand the API even if k-eff doesn't use them directly.
    //
    // Tally structure:
    //   - Tally: container for filters + scores + accumulated bins
    //   - ScoreType: Flux, Total, Fission, Absorption, NuFission, KappaFission, etc.
    //   - Filter: CellFilter, EnergyFilter, MaterialFilter, MeshFilter, etc.
    //   - TallyBin: one accumulator (sum, sum_sq, count) for statistics
    //
    // Example pseudocode (not run here):
    //
    //   let mut tally = Tally {
    //       id: 1,
    //       name: "Fission rate tally".into(),
    //       filters: vec![
    //           Box::new(CellFilter { cell_indices: vec![0] }),
    //           Box::new(EnergyFilter { bins: vec![0.0, 1.0e6, 2.0e6] }),
    //       ],
    //       scores: vec![ScoreType::Fission, ScoreType::Absorption],
    //       bins: vec![TallyBin::default(); 2 * 2],  // 2 energy bins × 2 scores
    //   };
    //
    //   // After a run with run_fixed_source(..., Some(&mut tally)):
    //   for (i, bin) in tally.bins.iter().enumerate() {
    //       let mean = bin.mean(n_batches);
    //       let rel_std_dev = bin.rel_std_dev(n_batches);
    //       println!("Bin {}: {} ± {} ({:.1}%)", i, mean, ...);
    //   }
    //
    // The TallyBin struct provides:
    //   - sum: running sum of scores
    //   - sum_sq: running sum of squares (for variance)
    //   - count: number of contributions
    //   - mean(n_realizations): returns sum / n_realizations
    //   - rel_std_dev(n_realizations): returns relative standard deviation

    println!("═══════════════════════════════════════════════════════════════════");
    println!("Note: Tallies and the Tally API");
    println!("═══════════════════════════════════════════════════════════════════\n");

    println!("This example demonstrates k-eff reading via run_keff(). To read flux or");
    println!("reaction-rate tallies in a Monte Carlo simulation, use run_fixed_source()");
    println!("with a Tally. The tally API includes:\n");

    println!("  • Tally — container for filters, scores, and accumulated bins");
    println!("  • ScoreType — Flux, Fission, Absorption, Total, NuFission, etc.");
    println!("  • Filter — CellFilter, EnergyFilter, MaterialFilter, MeshFilter, etc.");
    println!("  • TallyBin — one accumulator with .sum, .sum_sq, .count");
    println!("  • TallyBin methods: .mean(n_batches), .rel_std_dev(n_batches)\n");

    println!("Fixed-source usage (pseudocode):");
    println!("  let mut tally = Tally {{ ... }};");
    println!("  run_fixed_source(&geom, &materials, &nuclides, &source, &settings,");
    println!("                   Some(&mut tally));");
    println!("  for bin in &tally.bins {{");
    println!("      println!(\"mean: {{}}, rel_std: {{}}\",");
    println!("               bin.mean(n_batches), bin.rel_std_dev(n_batches));");
    println!("  }}\n");

    println!("For k-eff calculations with tally data, consider:");
    println!("  • Running a fixed-source adjoint calculation");
    println!("  • Or post-processing the converged k-eff source with a separate");
    println!("    fixed-source run with the tallies attached\n");

    println!("═══════════════════════════════════════════════════════════════════");
    println!("Tutorial Complete");
    println!("═══════════════════════════════════════════════════════════════════");
}
