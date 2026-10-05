//! **Majorant bound audit** — does `Majorant::bounding` actually bound
//! `Sigma_t` everywhere a history can reach?
//!
//! An under-bound majorant is a **silent** bias in delta tracking: the run
//! completes, nothing errors, and collisions are simply lost wherever
//! `Sigma_t > Sigma_maj`. Because the loss is concentrated exactly where the
//! cross section spikes — resonance peaks, and the 1/v rise at low energy —
//! it removes absorption and fission preferentially, which moves `k`.
//!
//! This scans a material table against its majorant on a fine logarithmic grid
//! and reports the worst ratio found. Diagnostic, not a gate.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example majorant_bound_audit
//! ```
//!
//! # GitHub #585 caller audit (2026-10-05)
//!
//! **Methodology.** Each `MAJORANT-AUDIT` row rebuilds one caller's majorant
//! with that caller's own materials and arguments, using
//! `pebble_beds::delta_tracking::bounding_audit_line`. It builds both the
//! pre-#585 construction (`bounding_without_breakpoints`, OLD) and the current
//! `bounding` (NEW), then runs `Majorant::audit`. The audit covers 2 000 001
//! log energies from 1e-5 eV to 20 MeV, every breakpoint with its one-ulp
//! neighbours, and every interval midpoint. Above 1 is an under-bound. The
//! LOW-tier rows are produced by this example. The ENDF/B-VIII.0 rows come
//! from each caller's own `OUTRAM_MAJORANT_AUDIT=1` hook (`dh_keff_vv` also
//! honours it). They were run on cores 2-3 of a 2.1 GHz Xeon.
//!
//! **Results.** OLD is from the run without WMP pole nodes, except where
//! marked. NEW includes pole nodes.
//!
//! | caller | data | args | OLD worst | NEW worst |
//! |---|---|---|---|---|
//! | `htr10_fuel_zone_kinf` (old ablation) | ENDF/B-VIII.0 + 30P | 4096x32, 0.1 | **1.1839** @ 1.689 MeV | 0.9092 |
//! | `godiva_*` (3 examples) | ENDF/B-VIII.0 | 4000x24, 0.10 | 0.9678 | 0.9091 |
//! | `thermal_kernel_keff_worth` (4 tabulations) | ENDF/B-VIII.0 + graphite | 4096x32, 0.1 | 0.9377 | 0.9166 |
//! | `htr10_pebble_delta_tracking` (5 boron arms) | ENDF/B-VIII.0 + graphite | `DhUniverse::keff`, 0.3 | 0.9989 @ 1.689 MeV | 0.7693 |
//! | `dh_keff_vv` (7 arms) | ENDF/B-VIII.0 FHR | `DhUniverse::keff`, 0.3 | ≤ 0.8970 | ≤ 0.7705 |
//! | `fhr_ring_rpt_endf` | ENDF/B-VIII.0 FHR | 4096x32, 0.3 | 0.8903 | 0.7693 |
//! | `triso_delta_tracking`, `tests/openmc_notebooks/triso.rs` | CORE (WMP) | 4096x32, 0.1 | 0.9434 | 0.9250 |
//! | `delta_tracking.rs` below-floor unit test | CORE | 2048x16, 0.1 | **1.2022** | 0.9356 |
//! | `outram-mc-tui` pebble presets (H and FLiBe) | CORE | 1024x16, 0.1 | **1.6200** | 0.9576 |
//! | `DhUniverse::keff` on `dh_thread_scaling`'s UCO pebble | CORE | 4096x32, 0.3 | **1.0084** (1.0115 with pole-node audit) @ 772 keV, SiC | 0.8243 |
//!
//! **Interpretation.** Every recorded ENDF/B-VIII.0 number was measured on a
//! majorant that bounded `Sigma_t`. The one exception is the HTR-10 fuel-zone
//! ablation run, which was never a result. `htr10_pebble_delta_tracking` held
//! by 0.1 %, and only because of its 30 % margin. The under-bounds are all on
//! CORE (WMP) data, at coarse settings or in Si-28's fast resonances. None of
//! those callers records a transport number that rests on it: the TUI is
//! interactive, the unit test checks only the below-floor extrapolation, and
//! `dh_thread_scaling` records scaling and reproducibility (its k values are
//! stated not to be V&V numbers; the bit-identical delta pair does not depend
//! on the bound). `triso_gpu_benchmark` reads
//! ENDF/B-VII.1 through `net-fetch` and could not be audited here. Its numbers
//! were already marked superseded and pending a re-run (`op-jis`).

use outram_mc_libs::prelude::*;
use outram_mc_libs::material::material::NuclideComponent;
use outram_mc_libs::pebble_beds::delta_tracking::bounding_audit_line;

fn audit(name: &str, materials: &[Material], nuclides: &[Nuclide], margin: f64, lo: f64, hi: f64) {
    let maj = Majorant::bounding(materials, nuclides, lo, hi, 4096, 32, margin);

    // Scan wider than the majorant was built over, because a history is not
    // confined to that range -- which is the point of the exercise.
    let (scan_lo, scan_hi): (f64, f64) = (1.0e-6, 2.0e7);
    let n = 200_000;
    let mut worst = 0.0_f64;
    let mut worst_e = 0.0;
    let mut worst_mat = 0usize;
    let mut breached = 0usize;

    for i in 0..=n {
        let f = i as f64 / n as f64;
        let e = scan_lo * (scan_hi / scan_lo).powf(f);
        let m = maj.at(e);
        for (mi, mat) in materials.iter().enumerate() {
            let st = mat.macro_xs_total(e, nuclides);
            if !(m > 0.0) {
                continue;
            }
            let ratio = st / m;
            if ratio > 1.0 {
                breached += 1;
            }
            if ratio > worst {
                worst = ratio;
                worst_e = e;
                worst_mat = mi;
            }
        }
    }

    println!("\n{name}");
    println!(
        "  majorant built over [{lo:.1e}, {hi:.1e}] eV, margin {:.0} %",
        margin * 100.0
    );
    println!(
        "  scanned            [{scan_lo:.1e}, {scan_hi:.1e}] eV, {n} points x {} materials",
        materials.len()
    );
    println!(
        "  worst Sigma_t / Sigma_maj = {worst:.4}  at E = {worst_e:.3e} eV \
              in '{}'",
        materials[worst_mat].name
    );
    if worst > 1.0 {
        println!("  *** UNDER-BOUND at {breached} scan points. Delta tracking loses collisions");
        println!(
            "      wherever this holds, which biases k. Sigma_maj is short by a factor {worst:.3}."
        );
    } else {
        println!(
            "  bounded everywhere scanned (headroom {:.1} %)",
            100.0 * (1.0 / worst - 1.0)
        );
    }

    // Where does the majorant actually stop?
    for e in [1.0e-6, 1.0e-5, 1.0e-4, 1.0e-3, 0.0253] {
        let m = maj.at(e);
        let st: f64 = materials
            .iter()
            .map(|mt| mt.macro_xs_total(e, nuclides))
            .fold(0.0, f64::max);
        println!(
            "    E = {e:.1e} eV : Sigma_maj = {m:.4}  max Sigma_t = {st:.4}  ratio {:.3}",
            st / m.max(f64::MIN_POSITIVE)
        );
    }
}

/// Fine scan across the resolved-resonance region, where a binned majorant is
/// most likely to miss a narrow peak between its subsamples.
///
/// The log-uniform scan in `audit` spreads its points over 13 decades; U-238's
/// low-lying resonances are ~25 meV wide at 6.67 eV, so they occupy a vanishing
/// fraction of that range and can be stepped straight over. This walks
/// 0.1 eV - 10 keV at a resolution far finer than any resonance width.
fn resonance_scan(materials: &[Material], nuclides: &[Nuclide], margin: f64) {
    let maj = Majorant::bounding(materials, nuclides, 1.0e-4, 2.0e7, 4096, 32, margin);
    let (lo, hi): (f64, f64) = (1.0e-1, 1.0e4);
    let n = 3_000_000; // ~1.5e-5 relative spacing: ~1e-4 eV at the 6.67 eV peak
    let mut worst = 0.0_f64;
    let mut worst_e = 0.0;
    let mut breaches = 0usize;

    for i in 0..=n {
        let e = lo * (hi / lo).powf(i as f64 / n as f64);
        let m = maj.at(e);
        if !(m > 0.0) {
            continue;
        }
        for mat in materials {
            let ratio = mat.macro_xs_total(e, nuclides) / m;
            if ratio > 1.0 {
                breaches += 1;
            }
            if ratio > worst {
                worst = ratio;
                worst_e = e;
            }
        }
    }
    println!(
        "\nResolved-resonance fine scan, margin {:.0} %",
        margin * 100.0
    );
    println!("  {lo:.1e} - {hi:.1e} eV at {n} points (~1e-4 eV resolution at 6.67 eV)");
    println!("  worst Sigma_t / Sigma_maj = {worst:.4} at E = {worst_e:.5} eV");
    if worst > 1.0 {
        println!("  *** UNDER-BOUND at {breaches} points — delta tracking loses collisions");
        println!("      in the FUEL, which loses absorption and biases k HIGH.");
    } else {
        println!(
            "  bounded (headroom {:.1} %) — the majorant is not missing resonance peaks",
            100.0 * (1.0 / worst - 1.0)
        );
    }
}

fn main() {
    // ---- the openmc-notebook TRISO lattice case (tests/openmc_notebooks/triso.rs) ----
    let nuclides: Vec<Nuclide> = ["U234", "U235", "U238", "H1"]
        .iter()
        .map(|n| Nuclide::from_core(n).unwrap())
        .collect();
    let comp = |i: usize, d: f64| NuclideComponent {
        nuclide_idx: i,
        atom_density: d,
    };
    let materials = vec![
        Material {
            id: 1,
            name: "HEU kernel".into(),
            temperature: 293.6,
            components: vec![comp(0, 4.9184e-4), comp(1, 4.4994e-2), comp(2, 2.4984e-3)],
        },
        Material {
            id: 2,
            name: "H matrix".into(),
            temperature: 293.6,
            components: vec![comp(3, 4.0e-2)],
        },
    ];

    println!("Majorant bound audit");
    println!("====================");
    audit(
        "openmc-notebook TRISO lattice (margin 0.1, as tests/openmc_notebooks/triso.rs builds it)",
        &materials,
        &nuclides,
        0.1,
        1.0e-4,
        2.0e7,
    );
    audit(
        "same materials, DH V&V settings (margin 0.3, floor 1e-4)",
        &materials,
        &nuclides,
        0.3,
        1.0e-4,
        2.0e7,
    );
    audit(
        "same materials, floor dropped to 1e-6 eV",
        &materials,
        &nuclides,
        0.1,
        1.0e-6,
        2.0e7,
    );
    resonance_scan(&materials, &nuclides, 0.1);

    // ---- GitHub #585 caller audit, LOW tier ----
    // Each row rebuilds a caller's majorant with that caller's own materials
    // and arguments, old (`bounding_without_breakpoints`) and new
    // (`bounding`), and audits both. The ENDF callers carry their own
    // `OUTRAM_MAJORANT_AUDIT` hook; see GitHub #585 for the full table.
    println!("\nGitHub #585 caller audit (LOW tier, embedded CORE data)");
    println!("=======================================================");
    let row = |label: &str, mats: &[Material], nucs: &[Nuclide], args: (f64, usize, usize, f64)| {
        let (floor, bins, sub, margin) = args;
        println!("{}", bounding_audit_line(label, mats, nucs, floor, 2.0e7, bins, sub, margin));
    };
    row(
        "triso_delta_tracking / tests/openmc_notebooks/triso.rs",
        &materials,
        &nuclides,
        (1.0e-4, 4096, 32, 0.1),
    );
    row(
        "delta_tracking.rs unit test (same materials)",
        &materials,
        &nuclides,
        (1.0e-4, 2048, 16, 0.1),
    );
    row(
        "outram-mc-tui preset, H matrix",
        &materials,
        &nuclides,
        (1.0e-4, 1024, 16, 0.1),
    );
    // The TUI's TMSR-like preset: the same kernel in a Li-7/Be-9/F-19 salt.
    let mut salt_nucs: Vec<Nuclide> = nuclides[..3].to_vec();
    for n in ["Li7", "Be9", "F19"] {
        salt_nucs.push(Nuclide::from_core(n).unwrap());
    }
    let nf = 0.0118143;
    let salt = vec![
        materials[0].clone(),
        Material {
            id: 2,
            name: "FLiBe-like salt (Li-7 only)".into(),
            temperature: 293.6,
            components: vec![comp(3, 2.0 * nf), comp(4, nf), comp(5, 4.0 * nf)],
        },
    ];
    row(
        "outram-mc-tui preset, FLiBe matrix",
        &salt,
        &salt_nucs,
        (1.0e-4, 1024, 16, 0.1),
    );
    // dh_thread_scaling / DhUniverse tests: the UCO pebble on CORE data.
    let dh_nucs: Vec<Nuclide> = ["U235", "U238", "O16", "C0", "Si28"]
        .iter()
        .map(|n| Nuclide::from_core(n).unwrap())
        .collect();
    let dh = vec![
        Material {
            id: 0,
            name: "UCO kernel".into(),
            temperature: 293.6,
            components: vec![comp(0, 4.40e-3), comp(1, 1.77e-2), comp(2, 2.27e-2), comp(3, 9.10e-3)],
        },
        Material {
            id: 3,
            name: "SiC".into(),
            temperature: 293.6,
            components: vec![comp(4, 4.79e-2), comp(3, 4.79e-2)],
        },
        Material {
            id: 5,
            name: "matrix graphite".into(),
            temperature: 293.6,
            components: vec![comp(3, 8.53e-2)],
        },
    ];
    row(
        "DhUniverse::keff on dh_thread_scaling's UCO pebble",
        &dh,
        &dh_nucs,
        (1.0e-4, 4096, 32, 0.3),
    );
}
