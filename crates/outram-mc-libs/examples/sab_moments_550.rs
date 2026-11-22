// SPDX-License-Identifier: AGPL-3.0-only

//! **H in H2O S(a,b): moments of what each processed table samples** (GitHub
//! #550), to be set beside the direct integral of the ENDF tape in
//! `verification_and_validation/tutorial_rung4/sab_reference_550/sab_reference.py`.
//!
//! # Methodology
//!
//! For each incident energy `E`, outgoing `(E', mu)` pairs are drawn with
//! `ThermalScattering::sample_inelastic` from four tables of the same law at
//! 293.6 K, and the moments `<E'>`, `var(E')`, `<mu>`, `cov(E', mu)` are
//! reported with a batch-means (20 batches) standard error:
//!
//! - `endf`: `from_tape` (log-spaced emission grid, `emission` nearest-record);
//! - `ace`: `from_ace` on NJOY2016's `HinH2O.ace` (the file OpenMC's `.h5` came
//!   from);
//! - `node`: `from_tape_on_calcem_grid` (emission tables on THERMR's grid);
//! - `fine`: `from_tape_with_grids` 1536 x 256 (the #550 arm D table).
//!
//! It also prints, with no sampling noise, the moments of the `calcem` node
//! tables themselves at node energies for 8 and 16 cosines per bin
//! (`emission` is exact at a node), and the nearest `calcem` node that the
//! log-spaced grid maps each probe energy to.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example sab_moments_550 -- \
//!     /path/to/293.6K/HinH2O.ace [n_samples]
//! ```
//!
//! Output is CSV on stdout; results are recorded in
//! `verification_and_validation/tutorial_rung4/sab_reference_550.md`.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf;
use njoy_outram_park_fork::thermr::scattering::IncoherentInelasticScattering;
use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
use outram_mc_libs::material::thermal::ThermalScattering;
use uom::si::energy::electronvolt;
use uom::si::thermodynamic_temperature::kelvin;

/// The five energies of the task, node energies (0.0253, 0.625 and the calcem
/// nodes 0.1035, 1.05, 3.42) and between-node energies.
const ENERGIES: [f64; 14] = [
    0.0253, 0.025, 0.08, 0.1, 0.1035, 0.15, 0.3, 0.4, 0.625, 0.9, 1.0, 1.05, 1.5, 3.5,
];

/// Moments of one batch of samples: `(mean E', var E', mean mu, cov(E', mu))`.
fn batch_moments(th: &ThermalScattering, e: f64, n: usize, seed: &mut u64) -> [f64; 4] {
    let (mut s1, mut s2, mut sm, mut sem) = (0.0, 0.0, 0.0, 0.0);
    let mut m = 0usize;
    for _ in 0..n {
        if let Some((eo, mu)) = th.sample_inelastic(e, seed) {
            s1 += eo;
            s2 += eo * eo;
            sm += mu;
            sem += eo * mu;
            m += 1;
        }
    }
    let mf = m as f64;
    let (mean, mum) = (s1 / mf, sm / mf);
    [mean, s2 / mf - mean * mean, mum, sem / mf - mean * mum]
}

/// Mean and batch-means standard error of each moment over `n_batches` batches.
fn sampled_moments(th: &ThermalScattering, e: f64, n: usize, n_batches: usize) -> ([f64; 4], [f64; 4]) {
    let mut seed = 987_654_321u64;
    let per = n / n_batches;
    let mut acc = vec![[0.0f64; 4]; n_batches];
    for b in acc.iter_mut() {
        *b = batch_moments(th, e, per, &mut seed);
    }
    let nb = n_batches as f64;
    let mut mean = [0.0; 4];
    let mut sem = [0.0; 4];
    for k in 0..4 {
        mean[k] = acc.iter().map(|b| b[k]).sum::<f64>() / nb;
        let var = acc.iter().map(|b| (b[k] - mean[k]).powi(2)).sum::<f64>() / (nb - 1.0);
        sem[k] = (var / nb).sqrt();
    }
    (mean, sem)
}

fn print_row(label: &str, e: f64, th: &ThermalScattering, n: usize) {
    let (m, s) = sampled_moments(th, e, n, 20);
    println!(
        "sampled,{label},{e},{:.8e},{:.2e},{:.8e},{:.2e},{:.8e},{:.2e},{:.8e},{:.2e}",
        m[0], s[0], m[1], s[1], m[2], s[2], m[3], s[3]
    );
}

/// Moments of a `calcem` node table, deterministic: equal weight on each
/// outgoing bin and each of its cosines.
fn table_moments(sab: &IncoherentInelasticScattering, e: f64, n_out: usize, n_cos: usize) -> [f64; 4] {
    let bins = sab.emission(NeutronEnergy::new::<electronvolt>(e), n_out, n_cos);
    let nb = bins.len() as f64;
    let (mut s1, mut s2, mut sm, mut sem) = (0.0, 0.0, 0.0, 0.0);
    for b in &bins {
        let eo = b.outgoing_energy.get::<electronvolt>();
        let mubar = b.cosines.iter().sum::<f64>() / b.cosines.len() as f64;
        s1 += eo;
        s2 += eo * eo;
        sm += mubar;
        sem += eo * mubar;
    }
    let (mean, mum) = (s1 / nb, sm / nb);
    [mean, s2 / nb - mean * mean, mum, sem / nb - mean * mum]
}

fn main() {
    let ace_path = std::env::args().nth(1).expect("path to HinH2O.ace");
    let n: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(4_000_000);
    let raw = njoy_outram_park_fork::acer::read::read(&ace_path).expect("read ACE");
    let ace = ThermalScattering::from_ace(&raw, "c_H_in_H2O").expect("from_ace");
    let tape_path = reference_endf("tsl-HinH2O.endf").expect("tape");
    let tape = Tape::read_file(&tape_path).expect("parse tape");
    let endf = ThermalScattering::from_tape(&tape, 1, 293.6, "c_H_in_H2O").expect("from_tape");
    let node = ThermalScattering::from_tape_on_calcem_grid(&tape, 1, 293.6, "c_H_in_H2O")
        .expect("node grid");
    let fine = ThermalScattering::from_tape_with_grids(&tape, 1, 293.6, "c_H_in_H2O", 1536, 256)
        .expect("fine");

    println!("kind,table,E_eV,mean_Ep,sem,var_Ep,sem,mean_mu,sem,cov_Ep_mu,sem");
    for &e in &ENERGIES {
        print_row("endf", e, &endf, n);
        print_row("ace", e, &ace, n);
        print_row("node", e, &node, n);
        print_row("fine", e, &fine, n);
    }

    // sigma_inel at the same energies, the three tables and the analytic integral.
    println!("kind,E_eV,sigma_endf,sigma_ace,sigma_node");
    for &e in &ENERGIES {
        println!("sigma,{e},{:.8e},{:.8e},{:.8e}", endf.inelastic_xs(e), ace.inelastic_xs(e), node.inelastic_xs(e));
    }

    // Node tables, no sampling: 8 against 16 cosines at the calcem nodes.
    let sab = IncoherentInelasticScattering::from_tape(&tape, 1, Temperature::new::<kelvin>(293.6))
        .expect("sab");
    println!("kind,E_eV,n_cos,mean_Ep,var_Ep,mean_mu,cov_Ep_mu");
    for &e in &[0.0253, 0.1035, 0.3011332, 0.625, 1.05, 3.42] {
        for &nc in &[8usize, 16] {
            let m = table_moments(&sab, e, 64, nc);
            println!("nodetable,{e},{nc},{:.8e},{:.8e},{:.8e},{:.8e}", m[0], m[1], m[2], m[3]);
        }
    }

    // Which calcem node does the nearest-record rule pick for a probe energy?
    let nodes = njoy_outram_park_fork::thermr::calcem::egrid::EGRID;
    println!("kind,E_eV,nearest_node,relative_offset");
    for &e in &ENERGIES {
        let j = (0..nodes.len())
            .min_by(|&a, &b| (nodes[a] - e).abs().partial_cmp(&(nodes[b] - e).abs()).unwrap())
            .unwrap();
        println!("nearest,{e},{},{:+.5}", nodes[j], (nodes[j] - e) / e);
    }
}
