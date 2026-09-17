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

use outram_mc_libs::prelude::*;
use outram_mc_libs::material::material::NuclideComponent;

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
}
