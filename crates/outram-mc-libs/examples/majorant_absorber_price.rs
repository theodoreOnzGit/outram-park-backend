//! **What does a control rod cost a global majorant?** — the measurement that
//! decides whether hybrid delta/surface tracking is worth building.
//!
//! # Why this exists
//!
//! The HTR-10 core model (`bn:op-867c`, gh #214) is to be solved with **delta
//! tracking inside the pebble bed and surface tracking everywhere else**. The
//! stated reason is that a strongly absorbing B4C control rod drives the
//! **majorant** up across the whole domain, so virtual-collision rejection
//! explodes even far from the rod, in graphite where nothing is happening.
//!
//! That argument was an **analogy**, not a measurement. This crate has measured
//! the pathology once before — bounding over the whole material table including
//! the undiluted fuel kernel multiplied virtual-collision counts by ~25x at the
//! U-238 resonances (`src/dh_universe.rs:101-108`) — but never for an absorber,
//! and never for HTR-10. Committing to a rewrite of two transport drivers on an
//! analogy is how a phase of work turns out to rest on nothing.
//!
//! # Method — why no transport is needed
//!
//! Delta tracking samples a flight as `s = -ln(xi) / Sigma_maj`, so the mean
//! free flight is `1 / Sigma_maj` and the expected number of tracking steps to
//! cross a fixed distance is **directly proportional to the majorant**. The
//! probability that any given step is a *virtual* collision is
//! `1 - Sigma_t(r,E) / Sigma_maj`.
//!
//! So the cost ratio between a bed-only majorant and a bed-plus-rod majorant is
//! exactly the **ratio of the two majorants**, and that is arithmetic on
//! `Majorant::from_materials` — no histories, no eigenvalue, no statistics.
//! Measuring it this way is not a shortcut; it is the quantity itself, free of
//! sampling noise.
//!
//! Both arms use the **identical** HTR-10 pebble from Li, Yu & Wei (2014)
//! Table 2, via `pebble_beds::htr10::fuel_pebble_materials`. The only
//! difference is whether a B4C rod material is present in the set the majorant
//! bounds. Delta tracking stays **unbiased** either way — an over-large
//! majorant costs time, never accuracy — which is precisely why the cost is the
//! whole question.
//!
//! # The rod
//!
//! Natural-boron B4C at 2.52 g/cm3 and full density, the usual absorber for an
//! HTGR control rod. That is an **illustrative** composition, not HTR-10's
//! specified rod, which is why this example reports a ratio and not an absolute
//! runtime: the conclusion is meant to survive the exact rod density being
//! different.
//!
//! # Results (2026-09-17) — the argument holds, and is now measured
//!
//! | energy | bed only | bed + rod | ratio |
//! |---|---|---|---|
//! | 1 meV | 20.04 | 551.69 | **27.5x** |
//! | 25.3 meV (thermal peak) | 4.18 | 109.86 | **26.3x** |
//! | 1 eV | 0.955 | 17.64 | **18.5x** |
//! | 6.67 eV (U-238 resonance) | 188.60 | 188.60 | 1.00x |
//! | 1 keV and above | — | — | **1.00x** |
//!
//! Worst over the full grid **27.8x at 1e-4 eV**; mean **11.3x**.
//!
//! **Interpretation.** A single control rod costs a global majorant a factor of
//! **~26x in tracking steps at thermal energies**, everywhere in the domain —
//! including in reflector graphite metres from the rod. That lands almost
//! exactly on the ~25x this crate already measured for the undiluted fuel
//! kernel (`dh_universe.rs:101-108`), so the B4C argument is not an analogy any
//! more; it is the same pathology, the same size, from a different cause.
//!
//! The **energy structure** is the useful part and was not predicted in
//! advance: the penalty is entirely thermal and epithermal, and is exactly
//! **1.00x from ~1 keV upward**. Above the resonance region the rod simply is
//! not the largest cross section in the problem, so it costs nothing. A
//! graphite-moderated pebble bed lives in precisely the range where the penalty
//! is worst, which is what makes this decisive for HTR-10 specifically rather
//! than for absorbers in general.
//!
//! Delta tracking remains **unbiased** under either majorant — `k` is
//! unaffected — so every bit of this is time, and every bit of it is what a
//! region-local majorant (`bn:op-867c.2`) recovers.
//!
//! **A correction made during this measurement.** The first run reported
//! **132x** at the thermal peak. That was wrong: the B-11 tape is absent from
//! `reference-data/endf/`, the loader silently substituted B-10, and the rod
//! came out ~5x too black. Natural boron is 19.9 at.% B-10 and the B-11 balance
//! is a non-absorber, so the substitution is now removed rather than flagged.
//! A rod five times too black makes the case for hybrid tracking look stronger
//! than it is, which is the one direction an author of this measurement must
//! not err in.
//!
//! ```bash
//! cargo run --release -p outram-mc-libs --example majorant_absorber_price
//! ```

use std::path::PathBuf;
use std::time::Instant;

use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading, Htr10Nuclides};

const TEMP_K: f64 = 300.15;

/// Index of each nuclide in the slice below.
const NUCLIDES: Htr10Nuclides = Htr10Nuclides {
    u235: 0,
    u238: 1,
    o16: 2,
    c_free: 3,
    c_graphite: 4,
    si28: 5,
    b10: 6,
};
// NOTE: there is deliberately NO B-11. `reference-data/endf/` carries only
// `n-005_B_010`, because B-11's ~5 mb capture against B-10's 3840 b is
// negligible — four parts in a million of natural boron's absorption. So the
// rod below carries natural boron's B-10 fraction and omits B-11 entirely,
// which is the physically correct treatment rather than a compromise.

fn reference_endf(file: &str) -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/endf")
        .join(file);
    p.exists().then_some(p)
}

fn nuclides() -> Option<Vec<Nuclide>> {
    let load = |name: &str, file: &str| -> Option<Nuclide> {
        let path = reference_endf(file)?;
        eprint!("  reconstructing {name:<6} from {file} … ");
        let t0 = Instant::now();
        let n = Nuclide::from_endf_file(&path, name, TEMP_K, 1.0e-3).ok()?;
        eprintln!("{:.1?}", t0.elapsed());
        Some(n)
    };
    let sab = ThermalScattering::from_endf_file(
        reference_endf("tsl-crystalline-graphite.endf")?.to_str()?,
        30,
        TEMP_K,
        "c_Graphite",
    )
    .ok()?;
    Some(vec![
        load("U235", "n-092_U_235-ENDF8.0.endf")?,
        load("U238", "n-092_U_238.endf")?,
        load("O16", "n-008_O_016-ENDF8.0.endf")?,
        load("C12", "n-006_C_012-ENDF8.0.endf")?,
        load("C12", "n-006_C_012-ENDF8.0.endf")?.with_thermal_scattering(sab),
        load("Si28", "n-014_Si_028-ENDF8.0.endf")?,
        load("B10", "n-005_B_010-ENDF8.0.endf")?,
    ])
}

/// Natural-boron B4C at 2.52 g/cm3 — an illustrative HTGR control-rod absorber.
///
/// Atom densities: M(B4C) with natural B (10.811) = 55.255 g/mol, so
/// N = 2.52 / 55.255 * 6.02214076e23 = 2.7465e22 /cm3 = 0.027465 /b/cm of B4C
/// units, giving 4x that in boron and 1x in carbon. Natural boron is
/// 19.9 at.% B-10, and the B-11 balance is omitted as a non-absorber (see the
/// note by the nuclide indices).
///
/// **A correction worth recording.** The first run of this example modelled the
/// whole boron inventory as B-10, because the B-11 tape is absent and the
/// loader silently substituted. That overstated the rod's absorption ~5x and
/// gave a 132x thermal ratio where the correct figure is far lower. The
/// substitution is now removed rather than flagged, because a rod that is 5x
/// too black makes the case for hybrid tracking look stronger than it is.
fn b4c_rod(id: i32) -> Material {
    const N_B4C: f64 = 0.0274654; // atoms/b·cm of B4C formula units
    const B10_ATOM_FRACTION: f64 = 0.199;
    let n_boron = 4.0 * N_B4C;
    Material {
        id,
        name: "B4C control rod (illustrative)".into(),
        components: vec![
            NuclideComponent {
                nuclide_idx: NUCLIDES.b10,
                atom_density: n_boron * B10_ATOM_FRACTION,
            },
            NuclideComponent {
                nuclide_idx: NUCLIDES.c_free,
                atom_density: N_B4C,
            },
        ],
        temperature: TEMP_K,
    }
}

/// Log grid over the full transport range, the same span `Majorant::bounding`
/// uses elsewhere in this crate.
fn energy_grid(n: usize) -> Vec<f64> {
    let (lo, hi) = (1.0e-4_f64, 2.0e7_f64);
    let (ll, lh) = (lo.ln(), hi.ln());
    (0..n)
        .map(|i| (ll + (lh - ll) * i as f64 / (n - 1) as f64).exp())
        .collect()
}

fn main() {
    println!("What a control rod costs a GLOBAL majorant");
    println!("=========================================");
    println!("  pebble : Li, Yu & Wei (2014) Table 2, natural boron impurity");
    println!("  rod    : B4C 2.52 g/cm3, natural B (19.9 at.% B-10), illustrative");
    println!("  data   : ENDF/B-VIII.0 + crystalline-graphite S(a,b), {TEMP_K} K\n");

    eprintln!("Reconstructing cross sections:");
    let Some(nucs) = nuclides() else {
        println!("SKIP: reference-data/endf/ tapes not available in this checkout.");
        return;
    };
    eprintln!();

    let pebble = fuel_pebble_materials(NUCLIDES, BoronReading::Natural, TEMP_K);
    let mut with_rod = pebble.clone();
    with_rod.push(b4c_rod(99));

    let grid = energy_grid(4096);
    let bed_only = Majorant::from_materials(&pebble, &nucs, &grid, 0.3);
    let bed_plus_rod = Majorant::from_materials(&with_rod, &nucs, &grid, 0.3);

    // Report at energies a reader can reason about, plus the extremes.
    let probes = [
        (1.0e-3, "1 meV      (below thermal)"),
        (0.0253, "25.3 meV   (thermal peak)"),
        (1.0, "1 eV       (epithermal)"),
        (6.67, "6.67 eV    (U-238 first resonance)"),
        (1.0e3, "1 keV      (resonance region)"),
        (1.0e5, "100 keV    (fast)"),
        (2.0e6, "2 MeV      (fission spectrum peak)"),
        (1.4e7, "14 MeV     (top of the range)"),
    ];

    println!(
        "{:<28} {:>14} {:>14} {:>10}",
        "energy", "bed only", "bed + rod", "ratio"
    );
    println!("{:-<70}", "");
    let mut worst = (0.0_f64, 0.0_f64);
    for (e, label) in probes {
        let a = bed_only.at(e);
        let b = bed_plus_rod.at(e);
        let ratio = if a > 0.0 { b / a } else { f64::NAN };
        if ratio > worst.1 {
            worst = (e, ratio);
        }
        println!("{label:<28} {a:>14.4} {b:>14.4} {ratio:>10.2}x");
    }

    // Sweep the whole grid for the true worst case, not just the probes.
    let mut sweep_worst = (0.0_f64, 1.0_f64);
    let mut weighted = 0.0_f64;
    let mut n = 0usize;
    for &e in &grid {
        let a = bed_only.at(e);
        if !(a > 0.0) {
            continue;
        }
        let r = bed_plus_rod.at(e) / a;
        if r > sweep_worst.1 {
            sweep_worst = (e, r);
        }
        weighted += r;
        n += 1;
    }

    println!(
        "\n  worst over the full grid : {:.2}x at {:.4e} eV",
        sweep_worst.1, sweep_worst.0
    );
    println!(
        "  mean over the full grid  : {:.2}x",
        weighted / n.max(1) as f64
    );
    println!(
        "\n  Read this as a COST multiplier on tracking steps everywhere in the\n  \
         domain, including in the reflector far from the rod. Delta tracking is\n  \
         unbiased under either majorant -- k is unaffected -- so this is purely\n  \
         the price of a global bound, and purely what a region-local majorant\n  \
         (bn:op-867c.2) would recover."
    );
}
