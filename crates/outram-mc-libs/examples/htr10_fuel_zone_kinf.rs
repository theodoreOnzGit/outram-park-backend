//! HTR-10 fuel-zone k_inf, doubly heterogeneous versus homogenised.
//!
//! Run it:
//! ```text
//! cargo run -p outram-mc-libs --release --example htr10_fuel_zone_kinf
//! # optional: particles-per-generation, inactive, active
//! cargo run -p outram-mc-libs --release --example htr10_fuel_zone_kinf -- 2000 25 100
//! # the recorded run (2026-10-05): see "Results" below
//! THREADS=1 cargo run -p outram-mc-libs --release --example htr10_fuel_zone_kinf -- 10000 50 200
//! ```
//!
//! Knobs: `THREADS=<n>` (rayon backend, default single-thread);
//! `OUTRAM_HTR10_GRAPHITE_TSL=crystalline|10P|30P` (graphite law, default 30P);
//! ablations `--low-tier` (the 2026-09-11 LOW-tier free-gas route) and
//! `OUTRAM_HTR10_MAJORANT=bounding` (the old majorant, audited but not
//! stopped on; its k is not a result).
//!
//! **Cost.** ~~Thermal neutrons in graphite scatter hundreds of times per
//! history, so this is far more expensive per particle than a fast system. The
//! defaults are sized for a few minutes; raise them for real statistics.~~
//! **Measured 2026-10-05:** about 0.28-0.30 ms per history per case on one core
//! of a 2.1 GHz Xeon (2.5 M histories in 704 s and 761 s), plus ~2.5 min of
//! ENDF processing (U-235 and U-238 ~70 s each, with URR tables) and a few
//! seconds per majorant. The defaults (400 x [15 + 45]) run in seconds.
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
//! - ~~**Thermal scattering in THIS EXAMPLE is FREE GAS.**~~ **CHANGED
//!   2026-10-05 (#528):** the default route now attaches the ENDF/B-VIII.0
//!   bound-graphite law (30 %-porous reactor graphite, MAT 32, the HTR-10
//!   choice recorded on `nee_soon`'s `GraphiteLaw`) to C-12 and C-13; free gas
//!   is the `--low-tier` ablation only. The UO2 kernel is still free gas (no
//!   U-in-UO2 / O-in-UO2 law here). The history below is kept as it was.
//!   ~~Graphite bound-atom S(alpha,beta) (coherent elastic Bragg plus
//!   incoherent elastic) does not reach the transport path in this
//!   workspace — `crates/outram-mc-libs/src/material/thermal.rs:24-26` says
//!   so explicitly.~~ **CORRECTED 2026-09-17** — that claim is false as of
//!   this date: `src/material/thermal.rs` implements the full bound-atom
//!   S(alpha,beta) treatment (incoherent inelastic MT=4, coherent elastic
//!   Bragg and incoherent elastic MT=2), `Nuclide::with_thermal_scattering`
//!   attaches it to a material (`src/material/nuclide.rs:358-374`), and both
//!   `pebble_beds/keff_delta.rs` (~:1023-1033) and `physics/transport_csg.rs`
//!   branch on `nuc.sample_thermal` in the collision kernel, exercised by
//!   `tests/thermal_graphite_elastic.rs`,
//!   `tests/htr10_graphite_thermal_scattering_pebble_bed.rs` and others. This
//!   *specific* example simply does not call
//!   `with_thermal_scattering` on its graphite `Nuclide`s (verified: no such
//!   call appears in this file) — free gas here is this example's own
//!   deliberate simplification, not a workspace-wide gap. Bead `op-hc2o`
//!   should be re-read/re-scoped rather than cited as "physics missing".
//!   On a graphite-moderated thermal system, running this example free-gas is
//!   still a first-order error in the thermal spectrum. **Every number this
//!   program prints is a code-exercise result, not a physics result.**
//!   Tracked as beads `op-hc2o`, `op-1y4y`, `op-6tz.35`.
//! - **The TRISO coatings are not resolved.** The buffer, inner PyC, SiC and
//!   outer PyC layers are smeared into the matrix graphite; only the fissile
//!   kernel is an explicit sphere. Tracked as bead `op-6tz.35`.
//! - ~~**Cross-section data is the LOW fidelity tier** — the embedded windowed-
//!   multipole CORE library with the 10-group fast fallback above its range,
//!   a flat nu-bar and a Watt fission-spectrum stand-in
//!   (`src/material/nuclide.rs:196-199`, `:1110-1124`; worth about +500 pcm on
//!   Godiva).~~ **CHANGED 2026-10-05 (#528):** the default is ENDF/B-VIII.0 read
//!   directly from `reference-data/endf/` (RECONR + BROADR at 293.15 K,
//!   tolerance 1e-3), with URR probability tables and DBRC applied by the
//!   constructor (`Nuclide::from_tape`, the correct-physics default); C is
//!   split into C-12/C-13 at 98.93/1.07 at.%. The LOW tier is the explicit
//!   `--low-tier` ablation. Published HTR-10 values are continuous-energy
//!   ENDF/B-VII.0.
//! - **Two open P1 RNG defects are inherited**: `op-rbo` (`init_seed` stream
//!   separation) and `op-jis` (missing PCG output permutation). `op-rbo` has no
//!   library call site so it does not touch this result, but `op-jis` means
//!   this crate cannot reproduce an OpenMC sequence bit-for-bit. (Not
//!   re-checked 2026-10-05.)
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
//! `iaea-tecdoc-1382-part2` (proprietary since 2026-09-22, held in the maintainer's private literature repository; see `crates/kovan-literature/CATALOGUE.md`),
//! markdown line 1101). They are not derived, fitted or invented here.
//!
//! The kernel radius (0.025 cm), fuelled-zone radius (2.5 cm) and particle
//! count per pebble (8335) come from the same document (Table 4-2 and its
//! Monte Carlo modelling notes), and are mirrored as typed data in
//! `outram_park_digital_twin_engine::htr10::neutronics`.

use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::keff_delta::run_keff_delta;
use outram_mc_libs::pebble_beds::sphere_packing::PackedSpheres;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::compute::{ComputeType, ThreadCount};
use njoy_outram_park_fork::reference_data::reference_endf;
use std::time::Instant;

// The fuel-zone model (TECDOC-1382 densities, layout, materials, majorant)
// lives in `common/htr10_fuel_zone.rs`, shared with the web demo's `htr10`
// rung (gh:#528), so the record and the browser build one model.
#[path = "common/htr10_fuel_zone.rs"]
mod fuel_zone;
use fuel_zone::*;

/// The 2026-09-11 data route: embedded WMP CORE library with its 10-group fast
/// fallback, free-gas carbon. Kept only as the explicit `--low-tier` ablation
/// so the old record can be reproduced.
fn low_tier_nuclides() -> (Vec<Nuclide>, Layout, String, String) {
    let nuclides = vec![
        Nuclide::from_core("U235").expect("U235 is in the embedded CORE library"),
        Nuclide::from_core("U238").expect("U238 is in the embedded CORE library"),
        Nuclide::from_core("O16").expect("O16 is in the embedded CORE library"),
        Nuclide::from_core("C0").expect("C-nat is in the embedded CORE library"),
        Nuclide::from_core("B10").expect("B10 is in the embedded CORE library"),
        Nuclide::from_core("B11").expect("B11 is in the embedded CORE library"),
    ];
    let layout = Layout {
        u235: 0,
        u238: 1,
        o16: 2,
        carbon: vec![(3, 1.0)],
        b10: 4,
        b11: 5,
    };
    (
        nuclides,
        layout,
        "LOW (ABLATION --low-tier: embedded WMP CORE + 10-group fast fallback)".into(),
        "FREE GAS (ABLATION --low-tier: no graphite S(alpha,beta))".into(),
    )
}

/// The default route: ENDF/B-VIII.0 read directly from `reference-data/endf/`
/// through this workspace's NJOY port (RECONR + BROADR at `temp_k`, tolerance
/// 1e-3), with the constructor's correct-physics defaults (URR probability
/// tables and DBRC, `Nuclide::from_tape`), C-12 and C-13 at natural
/// abundance, and bound-graphite S(alpha,beta) on both carbon isotopes.
///
/// The graphite law defaults to Hawari's 30 %-porous reactor graphite (MAT 32),
/// the maintainer's HTR-10 choice recorded on `nee_soon`'s `GraphiteLaw`
/// (porosity of 1.73 g/cm3 graphite is ~23 %, and 30 % is the nearer of the
/// two tabulated laws). `OUTRAM_HTR10_GRAPHITE_TSL=crystalline|10P|30P`
/// selects another, as an explicit ablation.
fn endf_nuclides(temp_k: f64) -> (Vec<Nuclide>, Layout, String, String) {
    let load = |name: &str, file: &str| -> Nuclide {
        let path = reference_endf(file)
            .unwrap_or_else(|| panic!("missing reference tape {file} in reference-data/endf/"));
        eprint!("  reconstructing {name:<6} from {file} … ");
        let t0 = Instant::now();
        let n = Nuclide::from_endf_file(&path, name, temp_k, 1.0e-3)
            .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", path.display()));
        eprintln!(
            "{:.1?}  (URR tables: {}, DBRC: {})",
            t0.elapsed(),
            n.urr_range_ev().is_some(),
            n.has_dbrc()
        );
        n
    };

    let law = std::env::var("OUTRAM_HTR10_GRAPHITE_TSL").unwrap_or_else(|_| "30P".into());
    let (tsl_file, tsl_mat) = match law.as_str() {
        "crystalline" => ("tsl-crystalline-graphite.endf", 30),
        "10P" => ("tsl-reactor-graphite-10P.endf", 31),
        "30P" => ("tsl-reactor-graphite-30P.endf", 32),
        other => panic!("OUTRAM_HTR10_GRAPHITE_TSL={other}: expected crystalline, 10P or 30P"),
    };
    let sab = ThermalScattering::from_endf_file(
        reference_endf(tsl_file)
            .unwrap_or_else(|| panic!("missing {tsl_file}"))
            .to_str()
            .expect("valid UTF-8 path"),
        tsl_mat,
        temp_k,
        "c_Graphite",
    )
    .expect("graphite S(alpha,beta)");
    let sab_t = sab.selected_temperature_k();
    eprintln!("  graphite S(alpha,beta): {tsl_file} MAT {tsl_mat}, table at {sab_t} K");

    // `common/htr10_fuel_zone.rs`'s tape list and layout (shared with the web
    // demo): U-235, U-238, O-16, C-12 and C-13 with the graphite law, B-10, B-11.
    let nuclides: Vec<Nuclide> = ENDF_TAPES
        .iter()
        .map(|&(name, file)| {
            let n = load(name, file);
            if name == "C12" || name == "C13" { n.with_thermal_scattering(sab.clone()) } else { n }
        })
        .collect();
    let layout = endf_layout();
    (
        nuclides,
        layout,
        "HIGH: ENDF/B-VIII.0 direct (RECONR + BROADR, tol 1e-3), URR + DBRC on".into(),
        format!("bound graphite S(alpha,beta) on C-12 and C-13: {tsl_file} (MAT {tsl_mat}) at {sab_t} K"),
    )
}

/// Check the delta-tracking majorant actually bounds `Sigma_t`, with
/// [`Majorant::audit`]. That covers 2 000 001 log-spaced energies from 1e-5 eV
/// to 20 MeV, plus every nuclide breakpoint with its one-ulp neighbours, plus
/// the midpoint between each pair of breakpoints. Each is checked against
/// `macro_xs_total_upper_bound` (the URR band maximum).
///
/// An under-bound majorant is a **silent** bias: collisions where
/// `Sigma_t > Sigma_maj` are never sampled. So this is checked rather than
/// assumed. It panics on a breach, because a run on an under-bound majorant
/// would be wrong without saying so.
fn audit_majorant(
    label: &str,
    materials: &[Material],
    nuclides: &[Nuclide],
    maj: &Majorant,
    fatal: bool,
) {
    let a = maj.audit(materials, nuclides, 1.0e-5, 2.0e7, 2_000_000);
    let (worst, worst_e) = (a.worst_ratio, a.energy_ev);
    let worst_m = &materials[a.material].name;
    println!(
        "Majorant audit: {label}: max Sigma_t/Sigma_maj = {worst:.4} at {worst_e:.4e} eV in \
         '{worst_m}' ({} energies: 2 000 001 log-spaced + every breakpoint, its \
         neighbours and the interval midpoints)",
        a.energies_checked
    );
    assert!(
        !fatal || worst <= 1.0,
        "the {label} majorant UNDER-BOUNDS Sigma_t by {:.2} % at {worst_e:.4e} eV in \
         '{worst_m}'; delta tracking would silently lose collisions there",
        (worst - 1.0) * 100.0
    );
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    // `--low-tier` reproduces the 2026-09-11 record (embedded WMP CORE data,
    // free-gas graphite). It is an explicit, visible ablation: the default is
    // the correct-physics ENDF route.
    let low_tier = argv.iter().any(|a| a == "--low-tier");
    let positional: Vec<&String> = argv.iter().filter(|a| !a.starts_with("--")).collect();

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

    // Benchmark core temperature for B1: 20 degrees Celsius = 293.15 K.
    let temperature_k = 293.15;

    let (nuclides, layout, data_label, thermal_label) = if low_tier {
        low_tier_nuclides()
    } else {
        endf_nuclides(temperature_k)
    };
    let (kernel, matrix, homogenised) =
        build_materials(&layout, temperature_k, kernel_packing_fraction);

    let packed = PackedSpheres::pack(
        KERNEL_RADIUS_CM,
        half,
        kernel_packing_fraction,
        packing_seed,
    )
    .expect("RSA packs well below its 0.38 ceiling at this fraction");

    println!("=== HTR-10 fuel-zone infinite medium — rung 1 step 1a ===");
    println!("Data          : IAEA-TECDOC-1382 Table 4-38 atom densities (Open tier)");
    println!("Fidelity tier : {data_label}");
    println!("Thermal       : {thermal_label}");
    println!("Coatings      : NOT resolved — buffer/IPyC/SiC/OPyC smeared into matrix");
    println!("Temperature   : {temperature_k} K (benchmark B1 core temperature, 20 C)");
    println!("Geometry      : reflective cube, half-width {half} cm, zero leakage (k_inf)");
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
    let arg = |i: usize, default: usize| -> usize {
        positional.get(i).and_then(|v| v.parse().ok()).unwrap_or(default)
    };
    // `THREADS=<n>` runs the rayon backend on n workers. Unset keeps the
    // single-thread reference backend (bit-reproducible for a fixed seed; the
    // multi-thread backend is reproducible for any thread count but does not
    // bit-match it).
    let compute = match std::env::var("THREADS").ok().and_then(|v| v.parse::<usize>().ok()) {
        Some(n) if n > 1 => ComputeType::CpuMultiThread(ThreadCount::Fixed(n)),
        _ => ComputeType::CpuSingleThread,
    };
    let settings = KeffSettings {
        n_particles: arg(0, 400),
        n_inactive: arg(1, 15),
        n_active: arg(2, 45),
        temperature_k,
        compute,
        ..KeffSettings::default()
    };
    println!(
        "Transport     : {} particles/generation, {} inactive + {} active, RNG seed {}, {:?}",
        settings.n_particles, settings.n_inactive, settings.n_active, transport_seed, settings.compute
    );
    println!();

    // --- Case 1: doubly heterogeneous, kernels resolved explicitly. ---
    let het_materials = vec![kernel, matrix];
    // `OUTRAM_HTR10_MAJORANT=bounding` is the explicit ablation: the old
    // pre-#585 `Majorant::bounding` construction, audited and reported but NOT stopped
    // on, so its under-bound can be measured. Its k is not a result.
    let old_majorant = std::env::var("OUTRAM_HTR10_MAJORANT").as_deref() == Ok("bounding");
    let majorant_for = |mats: &[Material]| {
        if old_majorant {
            Majorant::bounding_without_breakpoints(mats, &nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.1)
        } else {
            build_majorant(mats, &nuclides)
        }
    };
    if old_majorant {
        println!("ABLATION OUTRAM_HTR10_MAJORANT=bounding: old majorant, audit not fatal");
    }
    let het_majorant = majorant_for(&het_materials);
    audit_majorant("heterogeneous", &het_materials, &nuclides, &het_majorant, !old_majorant);
    let material_at = move |p: Position| {
        Some(if packed.is_inside_kernel(p) {
            0usize
        } else {
            1usize
        })
    };
    let t_het = Instant::now();
    let het = run_keff_delta(
        half,
        &het_materials,
        &nuclides,
        &het_majorant,
        material_at,
        &settings,
    );
    println!(
        "heterogeneous (kernels explicit) : k_inf = {:.5} +/- {:.5}   ({:.1} s)",
        het.k_mean,
        het.k_std,
        t_het.elapsed().as_secs_f64()
    );

    // --- Case 2: homogenised, identical nuclide inventory. ---
    let hom_materials = vec![homogenised];
    let hom_majorant = majorant_for(&hom_materials);
    audit_majorant("homogenised", &hom_materials, &nuclides, &hom_majorant, !old_majorant);
    let t_hom = Instant::now();
    let hom = run_keff_delta(
        half,
        &hom_materials,
        &nuclides,
        &hom_majorant,
        |_p: Position| Some(0usize),
        &settings,
    );
    println!(
        "homogenised   (same atoms)       : k_inf = {:.5} +/- {:.5}   ({:.1} s)",
        hom.k_mean,
        hom.k_std,
        t_hom.elapsed().as_secs_f64()
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
         fuel-zone infinite medium with unresolved TRISO coatings and no pebble\n\
         shell, moderator ball, reflector or leakage. It measures one\n\
         self-comparison; it does not validate anything.\n\
         See docs/reactor-scoping/htr10-neutronics.md sections 4.1 and 7.2."
    );

    vv_gate(
        het.k_mean,
        het.k_std,
        hom.k_mean,
        hom.k_std,
        dk_pcm,
        dk_sigma_pcm,
    );
}

/// V&V gate: the **one** claim this program is entitled to make.
///
/// # What is deliberately NOT asserted: either absolute k_inf
///
/// There is no oracle for them. This is a fuel-zone infinite medium — no
/// graphite shell, no dummy ball, no reflector, no leakage — and **no published
/// HTR-10 value corresponds to that problem**. ~~The thermal scattering is free
/// gas rather than graphite S(alpha,beta), and the data is the LOW tier.~~
/// (Since 2026-10-05 the default is ENDF/B-VIII.0 with bound graphite; the
/// absolute k still has no oracle.) Pinning
/// an absolute k_inf here would manufacture a reference that does not exist, and
/// the number would then get quoted as an HTR-10 result, which is exactly what
/// this file's closing text spends a paragraph forbidding.
///
/// So the gate asserts the **difference** and nothing else. That is a
/// self-comparison: same nuclide inventory, same transport stack, same settings,
/// the TRISO kernels resolved in one case and smeared in the other.
///
/// # The claim
///
/// Resolving the kernels must give a **higher** k_inf than homogenising them.
/// Lumping the fuel shields the resonance absorber from the thermal flux — fewer
/// U-238 captures per fission — so the heterogeneous case is more reactive. This
/// is the double-heterogeneity effect, and it is the same physics
/// `examples/fhr_ring_rpt_endf.rs` asserts through its own `naive - explicit`
/// claim, on a different geometry and a different data tier.
///
/// A code that had quietly stopped resolving the TRISO kernels would return the
/// two cases equal. That is the failure this catches, and it is invisible to any
/// check on an absolute k.
///
/// # Results — re-measured 2026-10-05, ENDF/B-VIII.0 + graphite S(alpha,beta)
///
/// **Methodology.** Default route (ENDF/B-VIII.0 direct, URR + DBRC on,
/// 30P graphite S(alpha,beta) on C-12/C-13, 293.15 K; the law's 296 K table is
/// used, within NJOY's `T/1000 + 5` K tolerance), union-grid majorant (the
/// example-local construction of f0d701bfc, since replaced by the library's;
/// see `build_majorant`) passing
/// the dense audit (worst `Sigma_t/Sigma_maj` = 0.9091 heterogeneous, 0.9092
/// homogenised, i.e. bounded at every node with the 10 % margin).
/// 10 000 histories x [50 inactive + 200 active] per case, RNG seed 1, packing
/// seed 20260811 (1018 kernels, realised f = 0.008328), `THREADS=1`
/// (single-thread backend). Binary built from `f0d701bfc` (built locally as
/// `5eb40da10` before the rebase onto `develop`; same content); hardware Intel Xeon @ 2.10 GHz (KVM, 4 vCPU, 260 MiB
/// L3, 15 GiB), pinned to one core with `taskset`, Linux 6.18.44, rustc
/// 1.95.0, `--release`. Prediction written first, in
/// `verification_and_validation/tutorial_rung5/README.md` (commit
/// `61adc0a4a`).
///
/// ```text
///   heterogeneous (kernels explicit)   k_inf = 1.57136 +/- 0.00088   (703.6 s)
///   homogenised   (same atoms)         k_inf = 1.44684 +/- 0.00090   (760.9 s)
///   delta k (hom - het)                      -12452 +/- 126 pcm  (98.7 sigma)
///   delta rho (hom - het)                     -5477 +/- 56 pcm
/// ```
///
/// Resolving the TRISO kernels is worth **12 452 ± 126 pcm in k** on this
/// data, correctly signed. Against the prediction: the sign held; the
/// magnitude is inside the predicted 10 000-20 000 pcm range but **smaller**
/// than the LOW-tier value, where the prediction said slightly larger. The
/// move from the old number, +2191 ± 1174 pcm, is 1.9 sigma (the old run's
/// statistics dominate), so it is not resolved. The heterogeneous k fell by
/// 2535 ± 901 pcm from the LOW record (predicted: 1000-4000 lower, held); the
/// homogenised k fell by only 344 ± 752 pcm (predicted 1000-4000 lower,
/// **missed**; not resolved from zero). Full record:
/// `verification_and_validation/tutorial_rung5/README.md`.
///
/// # Confirmatory re-run with the library majorant (GitHub #585, 2026-10-05)
///
/// The record above used the example-local union-grid majorant. It was
/// re-run on the library `Majorant::bounding` (`149f7aff0`; audit 0.9091 on
/// both cases) at the same settings, `THREADS=2` on cores 2-3. Prediction,
/// posted on #585 first: each k moves by under ~250 pcm (2 sigma of two
/// independent runs), and delta k stays inside 2 sigma.
///
/// ```text
///   heterogeneous   k_inf = 1.57287 +/- 0.00083   (608.0 s, 2 threads)
///   homogenised     k_inf = 1.44795 +/- 0.00085   (479.3 s, 2 threads)
///   delta k (hom - het)     -12493 +/- 119 pcm  (105.4 sigma)
/// ```
///
/// Movement against the record: heterogeneous +151 ± 121 pcm (1.2 sigma),
/// homogenised +111 ± 124 pcm (0.9 sigma), delta k −41 ± 173 pcm
/// (0.2 sigma). The prediction held. The majorant change moves nothing
/// resolvable, as expected of two valid bounds. The predicted 0–10 %
/// slowdown was **not testable**: the thread counts differ. The record
/// stands as the quoted number.
///
/// # Results (2026-09-11, LOW tier, free-gas thermal, reflective cube) — superseded
///
/// Kept as the record; superseded by the ENDF re-measurement above.
///
/// ```text
///   heterogeneous (kernels explicit)   k_inf = 1.59671 +/- 0.00897
///   homogenised   (same atoms)         k_inf = 1.45028 +/- 0.00747
///   delta k (hom - het)                      -14643 +/- 1167 pcm  (12.5 sigma)
/// ```
///
/// Resolving the TRISO kernels is worth ~~**14 643 pcm**~~ here (LOW tier,
/// superseded), correctly signed and at 12.5 sigma. Neither absolute number is an HTR-10 result and neither is
/// asserted; only the difference is.
///
/// # Tolerance
///
/// The effect must be present at **> 3 sigma** of the run's own combined
/// counting statistics, and in the right direction. No magnitude is pinned: it
/// depends on packing fraction, kernel radii and the data tier, none of which
/// have an external reference here either. ~~The 12.5 sigma measured leaves a
/// factor of four over the bar~~ The 98.7 sigma measured on 2026-10-05 (12.5
/// on the LOW-tier record) leaves a wide margin over the bar, so the gate
/// fires on the effect disappearing rather than on ordinary fluctuation. At
/// the default 400 x [15 + 45] the combined sigma is of order 1000 pcm (scaled
/// from a 200 x [5 + 10] pilot at 3175 pcm), so the
/// effect still clears 3 sigma there.
fn vv_gate(k_het: f64, s_het: f64, k_hom: f64, s_hom: f64, dk_pcm: f64, dk_sigma_pcm: f64) {
    println!("\n=== V&V gate: the double-heterogeneity difference (self-comparison) ===");

    assert!(
        k_het.is_finite() && k_hom.is_finite() && k_het > 0.0 && k_hom > 0.0,
        "k_inf came out as het = {k_het}, hom = {k_hom}; at least one is not a \
         multiplication factor"
    );
    assert!(
        s_het > 0.0 && s_hom > 0.0,
        "one of the runs reported zero statistical uncertainty (het {s_het}, hom \
         {s_hom}), so the significance test below would be meaningless"
    );

    println!("  heterogeneous {k_het:.5} +/- {s_het:.5}   homogenised {k_hom:.5} +/- {s_hom:.5}");
    println!(
        "  delta k (hom - het) = {dk_pcm:+.0} +/- {dk_sigma_pcm:.0} pcm ({:.1} sigma)",
        dk_pcm.abs() / dk_sigma_pcm
    );

    assert!(
        dk_pcm < 0.0,
        "homogenising the TRISO kernels RAISED k_inf by {dk_pcm:+.0} pcm. Smearing \
         the fuel through the matrix removes the resonance self-shielding the \
         kernels provide, so it must LOWER k, not raise it. Sign reversed means \
         the two cases are not the inventory-matched pair this comparison assumes."
    );
    assert!(
        dk_pcm.abs() > 3.0 * dk_sigma_pcm,
        "the double-heterogeneity difference is {dk_pcm:+.0} +/- {dk_sigma_pcm:.0} \
         pcm, only {:.1} sigma. The two cases carry the same nuclide inventory and \
         differ ONLY in whether the TRISO kernels are resolved, so an \
         indistinguishable result means the kernels are not being resolved at all \
         — the geometry is being built but not tracked through.\n\
         No absolute k is asserted here and none should be: there is no published \
         HTR-10 value for a fuel-zone infinite medium. This difference is the only \
         claim this program is entitled to make, so it is the only one gated.",
        dk_pcm.abs() / dk_sigma_pcm,
    );
    println!(
        "  [PASS] resolving the kernels is worth {:.0} pcm ({:.1} sigma) — the \
         double-heterogeneity effect is present and correctly signed",
        dk_pcm.abs(),
        dk_pcm.abs() / dk_sigma_pcm
    );
}
