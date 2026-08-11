//! HTR-10 fuel-zone k_inf, doubly heterogeneous versus homogenised.
//!
//! Run it:
//! ```text
//! cargo run -p outram-mc-libs --release --example htr10_fuel_zone_kinf
//! # optional: particles-per-generation, inactive, active
//! cargo run -p outram-mc-libs --release --example htr10_fuel_zone_kinf -- 2000 25 100
//! ```
//!
//! **Cost warning.** Thermal neutrons in graphite scatter hundreds of times per
//! history, so this is far more expensive per particle than a fast system. The
//! defaults are sized for a few minutes; raise them for real statistics.
//!
//! # What this computes, and what it does NOT
//!
//! This is **rung 1 step 1a** of the HTR-10 multifidelity pipeline scoped in
//! `docs/reactor-scoping/htr10-neutronics.md`: an infinite-medium k_inf of the
//! *fuelled zone* of an HTR-10 fuel pebble, run twice —
//!
//! 1. with the UO2 kernels resolved **explicitly** as randomly packed spheres
//!    (the level-1 double heterogeneity), and
//! 2. with exactly the same nuclide inventory **homogenised** into one medium.
//!
//! The difference between the two is this project's own measurement of the
//! **geometric self-shielding worth of lumping the fuel**: resolving the
//! kernels depresses the flux inside each lump at the U-238 resonance
//! energies, so fewer resonance absorptions occur per U-238 atom and k rises.
//! Homogenising removes that depression, so the homogenised case must come out
//! *lower*. The sign is therefore a physics check on the calculation.
//!
//! **This is related to, but NOT the same quantity as, the unit-cell biases
//! Wang et al. (2014) report.** Theirs are multigroup *cross-section
//! processing* biases against a continuous-energy reference on the full HTR-10
//! model, and they run the other way (+2820 pcm for INFHOMMEDIUM). Ours is a
//! continuous-energy *geometric* effect on a fuel-zone infinite medium. Do not
//! compare the two numbers directly.
//!
//! **It is NOT an HTR-10 criticality result, and must never be quoted as one.**
//! Specifically:
//!
//! - **It is a fuel-zone infinite medium**, not a pebble, not a pebble bed and
//!   not a core. There is no graphite shell, no dummy ball, no reflector and no
//!   leakage. No published HTR-10 value corresponds to this problem, so the
//!   absolute k_inf here cannot be compared to the literature. Only the
//!   heterogeneous-versus-homogeneous *difference* is meaningful, and only
//!   as a self-comparison.
//! - **Thermal scattering is FREE GAS.** Graphite bound-atom S(alpha,beta)
//!   (coherent elastic Bragg plus incoherent elastic) does not reach the
//!   transport path in this workspace —
//!   `crates/outram-mc-libs/src/material/thermal.rs:24-26` says so explicitly.
//!   On a graphite-moderated thermal system that is a first-order error in the
//!   thermal spectrum. **Every number this program prints is a code-exercise
//!   result, not a physics result.** Tracked as beads `op-hc2o`, `op-1y4y`,
//!   `op-6tz.35`.
//! - **The TRISO coatings are not resolved.** The buffer, inner PyC, SiC and
//!   outer PyC layers are smeared into the matrix graphite; only the fissile
//!   kernel is an explicit sphere. Tracked as bead `op-6tz.35`.
//! - **Cross-section data is the LOW fidelity tier** — the embedded windowed-
//!   multipole CORE library with the 10-group fast fallback above its range,
//!   a flat nu-bar and a Watt fission-spectrum stand-in
//!   (`src/material/nuclide.rs:196-199`, `:1110-1124`; worth about +500 pcm on
//!   Godiva). Published HTR-10 values are continuous-energy ENDF/B-VII.0.
//! - **Two open P1 RNG defects are inherited**: `op-rbo` (`init_seed` stream
//!   separation) and `op-jis` (missing PCG output permutation). `op-rbo` has no
//!   library call site so it does not touch this result, but `op-jis` means
//!   this crate cannot reproduce an OpenMC sequence bit-for-bit.
//!
//! # Reproducibility
//!
//! Everything needed to re-run this is printed by the program itself: the RNG
//! seed, the packing seed, the particle and generation counts, the fidelity
//! tier, the thermal-scattering treatment, and the realized packing fraction.
//! The sequential backend is bit-reproducible for a fixed seed.
//!
//! # Data provenance
//!
//! Atom densities are taken directly from **IAEA-TECDOC-1382**, *Evaluation of
//! high temperature gas cooled reactor performance: Benchmark analysis related
//! to initial testing of the HTTR and HTR-10*, IAEA Vienna, November 2003,
//! Table 4-38 (Open tier; catalogued at
//! `crates/kovan-literature/open/reports/iaea-tecdoc-1382-part2.json`,
//! markdown line 1101). They are not derived, fitted or invented here.
//!
//! The kernel radius (0.025 cm), fuelled-zone radius (2.5 cm) and particle
//! count per pebble (8335) come from the same document (Table 4-2 and its
//! Monte Carlo modelling notes), and are mirrored as typed data in
//! `outram_park_digital_twin_engine::htr10::neutronics`.

use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::keff_delta::run_keff_delta;
use outram_mc_libs::pebble_beds::sphere_packing::PackedSpheres;
use outram_mc_libs::physics::keff::KeffSettings;

/// UO2 kernel atom densities, atoms/barn-cm (IAEA-TECDOC-1382 Table 4-38).
const KERNEL_U235: f64 = 3.992067e-3;
const KERNEL_U238: f64 = 1.924449e-2;
const KERNEL_O16: f64 = 4.647329e-2;
const KERNEL_B10: f64 = 1.849637e-8;
const KERNEL_B11: f64 = 7.445022e-8;

/// Matrix graphite atom densities, atoms/barn-cm (IAEA-TECDOC-1382 Table 4-38).
/// The carbon figure corresponds to the specified 1.73 g/cm^3 graphite.
const MATRIX_C: f64 = 8.674169e-2;
const MATRIX_B10: f64 = 2.244010e-8;
const MATRIX_B11: f64 = 9.032424e-8;

/// Fuel kernel radius, cm (IAEA-TECDOC-1382 Table 4-2: 0.25 mm).
const KERNEL_RADIUS_CM: f64 = 0.025;
/// Fuelled-zone radius of a fuel pebble, cm (5.0 cm diameter).
const FUEL_ZONE_RADIUS_CM: f64 = 2.5;
/// Coated particles per fuel pebble (IAEA-TECDOC-1382 Monte Carlo notes).
const PARTICLES_PER_PEBBLE: f64 = 8335.0;

fn main() {
    // Kernel volume fraction of the fuelled zone:
    //   n * (4/3) pi r_k^3 / ((4/3) pi R_fz^3) = n * (r_k / R_fz)^3.
    let kernel_packing_fraction =
        PARTICLES_PER_PEBBLE * (KERNEL_RADIUS_CM / FUEL_ZONE_RADIUS_CM).powi(3);

    // A 2 cm cube (half-width 1 cm) holds about a thousand kernels at this
    // packing fraction - enough for a representative stochastic realisation
    // while staying well inside RSA's 0.38 ceiling.
    let half = 1.0;
    let packing_seed = 20260811;
    let transport_seed = KeffSettings::default().seed;

    let nuclides = vec![
        Nuclide::from_core("U235").expect("U235 is in the embedded CORE library"),
        Nuclide::from_core("U238").expect("U238 is in the embedded CORE library"),
        Nuclide::from_core("O16").expect("O16 is in the embedded CORE library"),
        Nuclide::from_core("C0").expect("C-nat is in the embedded CORE library"),
        Nuclide::from_core("B10").expect("B10 is in the embedded CORE library"),
        Nuclide::from_core("B11").expect("B11 is in the embedded CORE library"),
    ];

    // Benchmark core temperature for B1: 20 degrees Celsius = 293.15 K.
    let temperature_k = 293.15;

    let kernel = Material {
        id: 1,
        name: "HTR-10 UO2 kernel (17 wt% enriched)".into(),
        temperature: temperature_k,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: KERNEL_U235 },
            NuclideComponent { nuclide_idx: 1, atom_density: KERNEL_U238 },
            NuclideComponent { nuclide_idx: 2, atom_density: KERNEL_O16 },
            NuclideComponent { nuclide_idx: 4, atom_density: KERNEL_B10 },
            NuclideComponent { nuclide_idx: 5, atom_density: KERNEL_B11 },
        ],
    };
    let matrix = Material {
        id: 2,
        name: "HTR-10 graphite matrix (1.73 g/cm^3, 1.3 ppm EBC)".into(),
        temperature: temperature_k,
        components: vec![
            NuclideComponent { nuclide_idx: 3, atom_density: MATRIX_C },
            NuclideComponent { nuclide_idx: 4, atom_density: MATRIX_B10 },
            NuclideComponent { nuclide_idx: 5, atom_density: MATRIX_B11 },
        ],
    };

    // The homogenised counterpart: exactly the same nuclide inventory, volume
    // weighted into a single medium. Same atoms, no geometry.
    let f = kernel_packing_fraction;
    let homogenised = Material {
        id: 3,
        name: "HTR-10 fuel zone, homogenised".into(),
        temperature: temperature_k,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: KERNEL_U235 * f },
            NuclideComponent { nuclide_idx: 1, atom_density: KERNEL_U238 * f },
            NuclideComponent { nuclide_idx: 2, atom_density: KERNEL_O16 * f },
            NuclideComponent { nuclide_idx: 3, atom_density: MATRIX_C * (1.0 - f) },
            NuclideComponent {
                nuclide_idx: 4,
                atom_density: KERNEL_B10 * f + MATRIX_B10 * (1.0 - f),
            },
            NuclideComponent {
                nuclide_idx: 5,
                atom_density: KERNEL_B11 * f + MATRIX_B11 * (1.0 - f),
            },
        ],
    };

    let packed = PackedSpheres::pack(KERNEL_RADIUS_CM, half, kernel_packing_fraction, packing_seed)
        .expect("RSA packs well below its 0.38 ceiling at this fraction");

    println!("=== HTR-10 fuel-zone infinite medium — rung 1 step 1a ===");
    println!("Data          : IAEA-TECDOC-1382 Table 4-38 atom densities (Open tier)");
    println!("Fidelity tier : LOW (embedded WMP CORE + 10-group fast fallback)");
    println!("Thermal       : FREE GAS — graphite S(alpha,beta) NOT applied (op-hc2o)");
    println!("Coatings      : NOT resolved — buffer/IPyC/SiC/OPyC smeared into matrix");
    println!("Temperature   : {temperature_k} K (benchmark B1 core temperature, 20 C)");
    println!(
        "Geometry      : reflective cube, half-width {half} cm, zero leakage (k_inf)"
    );
    println!(
        "Packing       : {} kernels of r = {} cm, target f = {:.6}, realized f = {:.6}, seed {}",
        packed.len(),
        KERNEL_RADIUS_CM,
        kernel_packing_fraction,
        packed.packing_fraction(),
        packing_seed
    );

    // Particle and generation counts may be overridden from the command line
    // so the same example serves both a quick smoke run and a long statistics
    // run:  `--example htr10_fuel_zone_kinf -- <particles> <inactive> <active>`.
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize, default: usize| -> usize {
        argv.get(i).and_then(|v| v.parse().ok()).unwrap_or(default)
    };
    let settings = KeffSettings {
        n_particles: arg(0, 400),
        n_inactive: arg(1, 15),
        n_active: arg(2, 45),
        ..KeffSettings::default()
    };
    println!(
        "Transport     : {} particles/generation, {} inactive + {} active, RNG seed {}",
        settings.n_particles, settings.n_inactive, settings.n_active, transport_seed
    );
    println!();

    // --- Case 1: doubly heterogeneous, kernels resolved explicitly. ---
    let het_materials = vec![kernel, matrix];
    let het_majorant = Majorant::bounding(&het_materials, &nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.1);
    let material_at =
        move |p: Position| Some(if packed.is_inside_kernel(p) { 0usize } else { 1usize });
    let het = run_keff_delta(
        half,
        &het_materials,
        &nuclides,
        &het_majorant,
        material_at,
        &settings,
    );
    println!(
        "heterogeneous (kernels explicit) : k_inf = {:.5} +/- {:.5}",
        het.k_mean, het.k_std
    );

    // --- Case 2: homogenised, identical nuclide inventory. ---
    let hom_materials = vec![homogenised];
    let hom_majorant = Majorant::bounding(&hom_materials, &nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.1);
    let hom = run_keff_delta(
        half,
        &hom_materials,
        &nuclides,
        &hom_majorant,
        |_p: Position| Some(0usize),
        &settings,
    );
    println!(
        "homogenised   (same atoms)       : k_inf = {:.5} +/- {:.5}",
        hom.k_mean, hom.k_std
    );

    // Reactivity difference in pcm, with the combined statistical uncertainty.
    // rho_i = (k_i - 1)/k_i; Delta rho = rho_hom - rho_het, in pcm.
    let rho_het = (het.k_mean - 1.0) / het.k_mean;
    let rho_hom = (hom.k_mean - 1.0) / hom.k_mean;
    let d_rho_pcm = (rho_hom - rho_het) * 1.0e5;
    // d(rho)/dk = 1/k^2, so sigma_rho = sigma_k / k^2.
    let s_het = het.k_std / (het.k_mean * het.k_mean);
    let s_hom = hom.k_std / (hom.k_mean * hom.k_mean);
    let d_rho_sigma_pcm = (s_het * s_het + s_hom * s_hom).sqrt() * 1.0e5;
    let dk_pcm = (hom.k_mean - het.k_mean) * 1.0e5;
    let dk_sigma_pcm = (het.k_std * het.k_std + hom.k_std * hom.k_std).sqrt() * 1.0e5;

    println!();
    println!(
        "double-heterogeneity worth       : delta k = {:+.0} +/- {:.0} pcm, \
         delta rho = {:+.0} +/- {:.0} pcm (homogenised minus heterogeneous)",
        dk_pcm, dk_sigma_pcm, d_rho_pcm, d_rho_sigma_pcm
    );
    println!(
        "significance                     : {:.1} sigma",
        dk_pcm.abs() / dk_sigma_pcm
    );
    println!();
    println!(
        "READ THIS: neither k_inf above is an HTR-10 criticality result. This is a\n\
         fuel-zone infinite medium with free-gas thermal scattering and unresolved\n\
         TRISO coatings, on LOW-tier data. It exercises the rung-1 transport stack\n\
         end to end and measures one self-comparison; it does not validate anything.\n\
         See docs/reactor-scoping/htr10-neutronics.md sections 4.1 and 7.2."
    );
}
