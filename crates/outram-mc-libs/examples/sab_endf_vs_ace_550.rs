// SPDX-License-Identifier: AGPL-3.0-only

//! **H in H2O S(a,b): this crate's tape-built table against NJOY2016's ACE**
//! (GitHub #550, hypothesis H1b of the 2.40 cm diagnosis in
//! `verification_and_validation/tutorial_rung4/pitch_sweep.md`).
//!
//! # Methodology
//!
//! Two `ThermalScattering` tables for the same law, both at 293.6 K:
//! `ThermalScattering::from_endf_file` (the ENDF/B-VIII.0 `tsl-HinH2O.endf`
//! tape through this crate's THERMR-equivalent) and `ThermalScattering::from_ace`
//! on the NJOY2016 ACER `HinH2O.ace` that OpenMC's `c_H_in_H2O.h5` was made
//! from. The cross sections are compared on a log grid 1e-5 .. 4 eV (and on
//! each table's own breakpoints), and the first and second moments of the
//! outgoing energy are compared by sampling `sample_inelastic` at a few
//! incident energies (the mean energy transfer, which sets the thermal
//! spectrum). Report only; there is no pass criterion here, the gate is the
//! eigenvalue ensemble in `pitch_sweep.md`.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example sab_endf_vs_ace_550 -- \
//!     /path/to/HinH2O.ace
//! ```
//!
//! Results: `pitch_sweep.md`, section "2.40 cm miss: diagnosis".

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::thermal::ThermalScattering;

/// Mean and standard deviation of the outgoing energy and cosine from `n`
/// inelastic samples at incident energy `e` (own LCG-free seed stream: the
/// crate's sampler takes a `u64` state).
fn moments(th: &ThermalScattering, e: f64, n: usize, seed0: u64) -> (f64, f64, f64) {
    let mut seed = seed0;
    let (mut s_e, mut s_e2, mut s_mu) = (0.0, 0.0, 0.0);
    let mut m = 0usize;
    for _ in 0..n {
        if let Some((eo, mu)) = th.sample_inelastic(e, &mut seed) {
            s_e += eo;
            s_e2 += eo * eo;
            s_mu += mu;
            m += 1;
        }
    }
    let mf = m as f64;
    let mean = s_e / mf;
    (mean, (s_e2 / mf - mean * mean).max(0.0).sqrt(), s_mu / mf)
}

fn main() {
    let ace_path = std::env::args().nth(1).expect("path to HinH2O.ace");
    let raw = njoy_outram_park_fork::acer::read::read(&ace_path).expect("read ACE");
    let ace = ThermalScattering::from_ace(&raw, "c_H_in_H2O").expect("from_ace");
    let tape = reference_endf("tsl-HinH2O.endf").expect("H(H2O) tape");
    let endf = ThermalScattering::from_endf_file(tape.to_str().expect("path"), 1, 293.6, "c_H_in_H2O")
        .expect("from_endf_file");

    println!(
        "cutoff [eV]: ENDF-built {:.6}  ACE {:.6};  temperature [K]: ENDF-built {:.3}  ACE {:.3}",
        endf.cutoff_ev(),
        ace.cutoff_ev(),
        endf.selected_temperature_k(),
        ace.selected_temperature_k()
    );
    println!(
        "elastic: ENDF-built {}  ACE {};  skewed/continuous per table: see kind_name",
        endf.elastic().kind_name(),
        ace.elastic().kind_name()
    );

    println!("\nE [eV]      sigma_inel ENDF   sigma_inel ACE   rel diff     el ENDF   el ACE");
    let mut worst = (0.0f64, 0.0f64);
    let mut grid: Vec<f64> = (0..=80)
        .map(|i| 10f64.powf(-5.0 + 5.6 * i as f64 / 80.0))
        .filter(|e| *e < 4.0)
        .collect();
    grid.extend(endf.breakpoints());
    grid.extend(ace.breakpoints());
    grid.retain(|e| *e > 1e-5 && *e < 4.0);
    grid.sort_by(|a, b| a.partial_cmp(b).unwrap());
    grid.dedup();
    for (k, &e) in grid.iter().enumerate() {
        let (a, b) = (endf.inelastic_xs(e), ace.inelastic_xs(e));
        let rel = (a - b) / b.max(1e-30);
        if rel.abs() > worst.0.abs() {
            worst = (rel, e);
        }
        if k % 6 == 0 {
            println!(
                "{e:10.4e}  {a:15.6}  {b:15.6}  {rel:+10.3e}  {:8.4}  {:8.4}",
                endf.elastic_xs(e),
                ace.elastic_xs(e)
            );
        }
    }
    println!("\nworst relative sigma_inel difference over {} points: {:+.3e} at {:.4e} eV", grid.len(), worst.0, worst.1);

    // Weighted by a 293.6 K Maxwellian flux (E exp(-E/kT)) times sigma: a
    // reaction-rate-like integral, the thing eigenvalue sees.
    let kt = 8.617_333_262e-5 * 293.6;
    for (label, f) in [
        ("sigma_inel x Maxwellian flux", 0usize),
        ("sigma_inel x (1/E) flux 0.1..4 eV", 1usize),
    ] {
        let (mut ia, mut ib, mut wsum) = (0.0, 0.0, 0.0);
        let n = 20000;
        let (lo, hi) = if f == 0 { (1e-5f64, 0.5f64) } else { (0.1, 4.0) };
        for i in 0..n {
            let e = lo * (hi / lo).powf((i as f64 + 0.5) / n as f64);
            let w = if f == 0 { e * (-e / kt).exp() * e } else { 1.0 };
            ia += endf.inelastic_xs(e) * w;
            ib += ace.inelastic_xs(e) * w;
            wsum += w;
        }
        let _ = wsum;
        println!("{label}: ENDF-built / ACE - 1 = {:+.3e}", ia / ib - 1.0);
    }

    // Third table: the same tape on a 4x denser incident grid and 4x more
    // outgoing bins than the defaults (384 / 64), i.e. closer to the evaluation's
    // own kernel, to say which of the two is nearer to it.
    let tape_parsed = njoy_outram_park_fork::endf::tape::Tape::read_file(&tape).expect("tape");
    let fine =
        ThermalScattering::from_tape_with_grids(&tape_parsed, 1, 293.6, "c_H_in_H2O", 1536, 256)
            .expect("fine table");

    println!("\nOutgoing-energy moments of sample_inelastic (2e6 samples, same seed):");
    println!("E_in [eV]   <E'> ENDF   <E'> ACE   <E'> FINE   sd ENDF   sd ACE   sd FINE   <mu> ENDF  <mu> ACE  <mu> FINE");
    for e in [0.001, 0.0025, 0.01, 0.025, 0.05, 0.1, 0.3, 0.625, 1.5, 3.5] {
        let (m1, s1, u1) = moments(&endf, e, 2_000_000, 12345);
        let (m2, s2, u2) = moments(&ace, e, 2_000_000, 12345);
        let (m3, s3, u3) = moments(&fine, e, 2_000_000, 12345);
        println!(
            "{e:8.4}   {m1:9.5}  {m2:9.5}  {m3:9.5}  {s1:8.5}  {s2:8.5}  {s3:8.5}  {u1:+8.4}  {u2:+8.4}  {u3:+8.4}"
        );
    }
}
