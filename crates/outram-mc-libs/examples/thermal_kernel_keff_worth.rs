//! **What the S(α,β) emission-tabulation repair is worth in k** — measured as a
//! paired difference on a graphite-moderated thermal medium, with enough
//! histories to resolve it.
//!
//! # Why this exists
//!
//! `examples/thermal_emission_grid_convergence.rs` shows that the emission
//! tabulation was wrong (GitHub #190 / #188) and sizes the repair against an
//! NJOY oracle. It does not say what the repair is *worth*, and a kernel defect
//! that moves nothing is a different kind of finding from one that moves 300 pcm.
//!
//! The first attempt at that number, 2026-09-12, was
//! `examples/fhr_ring_rpt_endf.rs` run twice at 4000 × [30 + 80]: **−63 pcm with
//! σ ≈ 214 pcm on each side**. That is not a measurement, it is a
//! non-measurement — the quoted difference is a quarter of one standard error.
//! This program exists to do it properly.
//!
//! # How it gets a resolvable number
//!
//! Three things, none of them "run the pebble for a day":
//!
//! 1. **A cheaper system with no source-convergence problem.** A homogeneous
//!    graphite-moderated medium in a **reflective** box — `k_inf`, zero leakage,
//!    no TRISO packing, no CSG boundary crossings. The thermal kernel's
//!    influence on k runs through the thermal spectrum and the 0.625 eV
//!    crossing, and a homogeneous medium at a pebble's moderation ratio carries
//!    that just as the pebble does. A bare 200 cm sphere was tried first and
//!    rejected: its fission source does not converge in the inactive
//!    generations affordable here, and that moved k by ~2000 pcm between 500
//!    and 1000 particles per generation. A uniform medium with reflective
//!    boundaries has a flat fundamental mode and no such bias.
//! 2. **Both laws in one process.** The nuclides are reconstructed once
//!    (RECONR + BROADR is ~85 s and would otherwise be paid twice per replica),
//!    and only the S(α,β) table dimensions differ between the two runs —
//!    [`ThermalScattering::from_tape_with_grids`] makes that a two-argument
//!    change with nothing else moving.
//! 3. **Replicas, and the standard error of the DIFFERENCE.** Each replica runs
//!    every tabulation at its own master seed, and the reported uncertainty is
//!    the standard error of the mean of the per-replica *differences* — which is
//!    what a claim about Δk actually rests on, and which does not have to assume
//!    the two runs are independent.
//!
//!    **Common random numbers were tried and do not help here, which is worth
//!    recording.** The two runs of a replica start from the same source sites
//!    and the same per-history seed stream, and every thermal collision draws
//!    the same *number* of uniforms in both (channel split, table selection,
//!    outgoing bin, cosine), so the streams stay aligned. One might hope a
//!    single-generation paired run (`inactive = 0`, `active = 1`, many seeds)
//!    would then give a low-variance paired difference. Measured 2026-09-12 at
//!    4 x 10 000 histories it does not: the per-replica differences scattered by
//!    360–520 pcm, exactly the independent-sampling expectation. The reason is
//!    physical, not technical — a neutron in graphite scatters hundreds of times
//!    per history, so the two histories diverge at the *first* thermal collision
//!    and share nothing afterwards. There is no correlation left to exploit, and
//!    the cost of Δk here is brute force.
//!
//! # What this is NOT
//!
//! **Not an HTR-10, FHR or ICSBEP result, and not comparable to any published
//! k.** It is an infinite medium of invented but plausible composition; only the
//! *difference* between two thermal tabulations on the identical medium is
//! meaningful, and only as a self-comparison. Quoting the absolute k here as a
//! physics result would be wrong.
//!
//! # Running it
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example thermal_kernel_keff_worth -- [particles] [inactive] [active] [replicas]
//! ```
//!
//! # Results
//!
//! Recorded in the doc comment of
//! [`outram_mc_libs::vv::njoy_golden::GRAPHITE_KERNEL_WIDTH`], with the date and
//! the history count they took.

use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::keff_delta::run_keff_delta;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings};
use std::path::PathBuf;

/// Medium temperature \[K\] — the FHR/HTR pebble operating point, and a
/// *tabulated* temperature on `tsl-crystalline-graphite`, so no S(α,β)
/// temperature interpolation enters the comparison.
const TEMP_K: f64 = 600.0;

/// The four emission tabulations priced, as `(n_emit, n_outgoing, label)`.
///
/// The first is the superseded state GitHub #190 describes and the last is the
/// current default; the two in between isolate **which dimension** the k-worth
/// comes from, which a single before/after cannot say. The first row is the
/// reference every difference is taken against.
const CONFIGS: &[(usize, usize, &str)] = &[
    (48, 16, "48x16  (superseded)"),
    (384, 16, "384x16 (grid only)"),
    (48, 64, "48x64  (bins only)"),
    (384, 64, "384x64 (current)"),
];

/// Half-width \[cm\] of the **reflective** cube the medium fills.
///
/// Reflective on all six faces means **zero leakage**, so what is computed is
/// `k_inf` and the size is physically irrelevant — which is the point. A bare
/// sphere was tried first and is the wrong harness: it is a large, loosely
/// coupled system whose fission source needs many inactive generations to
/// converge, and an unconverged source moved k by ~2000 pcm between 500 and
/// 1000 particles per generation, swamping the effect being measured. A uniform
/// medium in a reflective box has a spatially flat fundamental mode, so the
/// source is converged from the first generation and the only uncertainty left
/// is the statistical one.
const HALF_CM: f64 = 20.0;

/// Carbon-to-heavy-metal atom ratio. ~400 puts the medium in the
/// over-moderated thermal regime a graphite pebble bed sits in, which is where
/// the thermal kernel has the most leverage on k.
const C_PER_HM: f64 = 400.0;

/// U-235 enrichment (HALEU, as in the FHR pebble deck).
const ENRICH: f64 = 0.1975;

const AVOGADRO_1E24: f64 = 0.60221408570;

mod nx {
    pub const U235: usize = 0;
    pub const U238: usize = 1;
    pub const O16: usize = 2;
    pub const C12G: usize = 3;
    pub const C13G: usize = 4;
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize, d: usize| a.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let (n_particles, n_inactive, n_active, n_replicas) =
        (arg(0, 4000), arg(1, 30), arg(2, 80), arg(3, 4));

    eprintln!(
        "=== k-worth of the S(a,b) emission-tabulation repair ===\n\
         graphite-moderated homogeneous medium, {TEMP_K} K, C/HM = {C_PER_HM}, \
         reflective cube half = {HALF_CM} cm (k_inf, zero leakage)\n\
         {n_replicas} replicas x {n_particles} particles x [{n_inactive} + {n_active}], \
         {} tabulations\n",
        CONFIGS.len()
    );

    let tape = {
        use njoy_outram_park_fork::endf::tape::Tape;
        let p = endf_dir().join("tsl-crystalline-graphite.endf");
        Tape::read(std::fs::File::open(&p).expect("tsl tape")).expect("parse tsl tape")
    };
    let base = base_nuclides();
    let material = medium();

    // One nuclide array per tabulation. Everything else — the reconstructed
    // cross sections, the medium, the seeds — is shared, so the only thing that
    // differs between two runs of the same replica is the S(a,b) tables.
    let mut sets: Vec<(String, Vec<Nuclide>)> = Vec::new();
    for &(n_emit, n_out, label) in CONFIGS {
        let t0 = std::time::Instant::now();
        let sab =
            ThermalScattering::from_tape_with_grids(&tape, 30, TEMP_K, "c_Graphite", n_emit, n_out)
                .expect("S(a,b) tables");
        eprintln!("  S(a,b) {label}: built in {:.1?}", t0.elapsed());
        let mut v = base.clone();
        v[nx::C12G] = v[nx::C12G].clone().with_thermal_scattering(sab.clone());
        v[nx::C13G] = v[nx::C13G].clone().with_thermal_scattering(sab);
        sets.push((label.to_string(), v));
    }

    // k[config][replica]
    let mut k: Vec<Vec<f64>> = vec![Vec::with_capacity(n_replicas); sets.len()];
    for r in 0..n_replicas {
        let settings = KeffSettings {
            n_particles,
            n_inactive,
            n_active,
            temperature_k: TEMP_K,
            seed: 20_260_912 + r as u64,
            compute: ComputeType::CpuMultiThread(Default::default()),
            ..KeffSettings::default()
        };
        eprint!("  replica {r}:");
        for (i, (label, nucs)) in sets.iter().enumerate() {
            // One majorant per nuclide set: the thermal tables change sigma_t
            // below 4 eV, so a majorant built for one tabulation is not
            // guaranteed to bound another.
            let majorant = Majorant::bounding(
                std::slice::from_ref(&material),
                nucs,
                1.0e-4,
                2.0e7,
                4096,
                32,
                0.1,
            );
            let res = run_keff_delta(
                HALF_CM,
                std::slice::from_ref(&material),
                nucs,
                &majorant,
                |_p: Position| Some(0usize),
                &settings,
            );
            eprint!(" {label} k={:.5}+/-{:.5};", res.k_mean, res.k_std);
            k[i].push(res.k_mean);
        }
        eprintln!();
    }

    let n = n_replicas as f64;
    println!(
        "\n{:>22} {:>10} {:>10} {:>14} {:>9} {:>8}",
        "tabulation", "k", "sigma_k", "delta vs 48x16", "sigma_d", "sigma"
    );
    for (i, (label, _)) in sets.iter().enumerate() {
        let mean = k[i].iter().sum::<f64>() / n;
        let sem = sem_of(&k[i]);
        if i == 0 {
            println!(
                "{label:>22} {mean:>10.5} {sem:>10.5} {:>14} {:>9} {:>8}",
                "-", "-", "-"
            );
            continue;
        }
        // Paired differences, replica by replica: the uncertainty on delta k is
        // the spread of the differences themselves, not the two k errors added
        // in quadrature.
        let d: Vec<f64> = k[i]
            .iter()
            .zip(&k[0])
            .map(|(a, b)| (a - b) * 1.0e5)
            .collect();
        let dm = d.iter().sum::<f64>() / n;
        let ds = sem_of(&d);
        println!(
            "{label:>22} {mean:>10.5} {sem:>10.5} {dm:>+14.1} {ds:>9.1} {:>8.1}",
            (dm / ds).abs()
        );
    }
    println!(
        "\n  delta k in pcm; {n_replicas} replicas x {} active histories each; \
         paired by master seed.",
        n_particles * n_active
    );
}

/// Standard error of the mean of `xs`. `NaN` for a single sample: with one
/// replica there is no spread to measure and the honest answer is "unknown",
/// not zero.
fn sem_of(xs: &[f64]) -> f64 {
    let n = xs.len() as f64;
    if xs.len() < 2 {
        return f64::NAN;
    }
    let m = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0);
    (var / n).sqrt()
}

fn endf_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("reference-data")
        .join("endf")
}

fn load(name: &str, file: &str) -> Nuclide {
    let p = endf_dir().join(file);
    eprint!("  reconstructing {name:<6} … ");
    let t0 = std::time::Instant::now();
    let n = Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
        .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
    eprintln!("{:.1?}", t0.elapsed());
    n
}

/// The five nuclides of the medium, RECONR + BROADR at [`TEMP_K`], with no
/// thermal law attached yet — the caller bolts on whichever S(α,β) tabulation
/// is being priced.
fn base_nuclides() -> Vec<Nuclide> {
    eprintln!("Reconstructing nuclides (RECONR + BROADR @ {TEMP_K} K):");
    vec![
        load("U235", "n-092_U_235-ENDF8.0.endf"),
        load("U238", "n-092_U_238.endf"),
        load("O16", "n-008_O_016-ENDF8.0.endf"),
        load("C12", "n-006_C_012-ENDF8.0.endf"),
        load("C13", "n-006_C_013-ENDF8.0.endf"),
    ]
}

/// UO₂ dispersed in graphite at [`C_PER_HM`] carbon atoms per heavy-metal atom.
///
/// Atom densities \[atoms/barn·cm\] are built from a graphite matrix at
/// 1.7 g/cm³ (the density of the bulk moderator, which dominates the volume) and
/// the heavy-metal loading that ratio implies. Plausible, not a specification —
/// see the module docs.
fn medium() -> Material {
    const M_C12: f64 = 12.0;
    const M_C13: f64 = 13.003355;
    const C12_AB: f64 = 0.9893;
    const C13_AB: f64 = 0.0107;
    let m_c = C12_AB * M_C12 + C13_AB * M_C13;
    let n_c = 1.7 * AVOGADRO_1E24 / m_c; // atoms/barn·cm of carbon
    let n_hm = n_c / C_PER_HM;
    Material {
        id: 1,
        name: "UO2 in graphite (homogeneous, C/HM = 400)".into(),
        temperature: TEMP_K,
        components: vec![
            NuclideComponent {
                nuclide_idx: nx::U235,
                atom_density: ENRICH * n_hm,
            },
            NuclideComponent {
                nuclide_idx: nx::U238,
                atom_density: (1.0 - ENRICH) * n_hm,
            },
            NuclideComponent {
                nuclide_idx: nx::O16,
                atom_density: 2.0 * n_hm,
            },
            NuclideComponent {
                nuclide_idx: nx::C12G,
                atom_density: C12_AB * n_c,
            },
            NuclideComponent {
                nuclide_idx: nx::C13G,
                atom_density: C13_AB * n_c,
            },
        ],
    }
}
