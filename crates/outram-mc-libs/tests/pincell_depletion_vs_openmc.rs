#![cfg(feature = "std")]
//! **OpenMC's pincell depletion example, OpenMC against this crate**
//! (gh:#1626 arm 2).
//!
//! # Methodology
//!
//! Case: OpenMC `e4febe9`'s `examples/pincell_depletion` (model, chain
//! `chain_simple.xml`, Predictor, five 1-day steps at 174 W/cm, 1000
//! particles x [10 + 90]), both codes on one ACE library made by this
//! workspace's NJOY port from ENDF/B-VIII.0 (RECONR 0.001, BROADR 293.6 K,
//! HEATR, PURR 20/64/10000, 0 K companions for every nuclide, H in H2O from
//! the LEAPR deck); OpenMC reads it after `IncidentNeutron.from_ace`
//! (`openmc_inputs/pincell_ace_to_hdf5.py`).
//!
//! - **OpenMC:** `openmc_inputs/ref_pincell_depletion.py`, 16 seeds in
//!   `reaction_rate_mode = "flux"` (CCFE-709) and 16 in the example's
//!   default `"direct"` mode; output `openmc_outputs/pincell_depletion.tsv`.
//! - **Port:** `examples/pincell_depletion_vs_openmc.rs`, 16 seeds, the
//!   flux-mode `depletion::coupled_operator::CoupledOperator` with PETIR's
//!   Predictor and CRAM-48, OpenMC's initial compositions; output
//!   `openmc_outputs/pincell_depletion_port.tsv`.
//!
//! Two checks:
//!
//! 1. **OpenMC's rates through the port's integrator** (no statistics):
//!    every OpenMC step `i -> i+1` (both modes, all seeds) recomputed from
//!    OpenMC's own beginning-of-step atoms and rates with the chain's
//!    `build_matrix` and CRAM-48. This isolates depletion from transport.
//! 2. **Transport plus depletion**: per step, the 16-seed mean of `k` and of
//!    the atoms of U-235, U-238, I-135, Xe-135, Xe-136, Cs-135, Gd-157,
//!    port against OpenMC flux mode, `z = d / sqrt(sem_a^2 + sem_b^2)`.
//!
//! # Predictions, written 2026-11-17 before the port was run
//!
//! 1. Check 1: within **1e-10** relative on every entry above 1e-12 of the
//!    vector's largest, every step (arms 1 and 3 reached ~1e-15).
//! 2. Check 2: **|z| <= 3** for `k` at each of the 6 records and for each
//!    listed nuclide at each of the 5 depleted records (41 comparisons; at
//!    3 sigma about 0.1 expected to exceed it by chance). Expected size of
//!    any real difference: under 100 pcm in `k` (the five-route study's
//!    thermal cases agree between the two codes on one library within
//!    statistics).
//! 3. OpenMC flux against direct mode, recorded only, not gated: the
//!    CCFE-709 collapse moves step-5 atoms by under 0.5 % for every listed
//!    nuclide.
//!
//! # Results
//!
//! Filled in after the run.

use njoy_outram_park_fork::hdf5::depletion_chain_xml::DepletionChainXml;
use outram_mc_libs::depletion::chain::DepletionChain;
use outram_mc_libs::depletion::cram::cram48;
use outram_mc_libs::depletion::{MicroRate, ReactionRates};
use std::collections::HashMap;
use std::path::PathBuf;

/// `(mode, seed, step)` -> nuclide -> value.
type Table = HashMap<(String, u64, usize), HashMap<String, f64>>;

#[derive(Default)]
struct Runs {
    k: HashMap<(String, u64, usize), (f64, f64)>,
    atoms: Table,
    gamma: Table,
    fission: Table,
}

fn read(path: &std::path::Path, runs: &mut Runs) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        match f.as_slice() {
            ["k", m, s, i, k, sd] => {
                runs.k.insert(((*m).into(), s.parse().unwrap(), i.parse().unwrap()), (k.parse().unwrap(), sd.parse().unwrap()));
            }
            ["atoms", m, s, i, n, x] => {
                runs.atoms.entry(((*m).into(), s.parse().unwrap(), i.parse().unwrap())).or_default().insert((*n).into(), x.parse().unwrap());
            }
            ["rate", m, s, i, n, rx, x] => {
                let t = if *rx == "fission" { &mut runs.fission } else { &mut runs.gamma };
                t.entry(((*m).into(), s.parse().unwrap(), i.parse().unwrap())).or_default().insert((*n).into(), x.parse().unwrap());
            }
            _ => {}
        }
    }
}

fn outputs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/spectrum_depletion/openmc_outputs")
}

fn chain() -> (DepletionChain, Vec<String>) {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/spectrum_depletion/openmc_inputs/chain_simple_tests.xml");
    let xml = DepletionChainXml::parse(&std::fs::read_to_string(p).unwrap()).unwrap();
    let c = DepletionChain::from_chain_xml(&xml, 0.0253).unwrap();
    let names = c.nuclide_names().iter().map(|s| (*s).to_owned()).collect();
    (c, names)
}

fn mean_sem(x: &[f64]) -> (f64, f64) {
    let n = x.len() as f64;
    let m = x.iter().sum::<f64>() / n;
    let var = x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / (n - 1.0);
    (m, (var / n).sqrt())
}

#[test]
fn openmc_rates_through_the_port_integrator() {
    let mut runs = Runs::default();
    read(&outputs().join("pincell_depletion.tsv"), &mut runs);
    let (chain, names) = chain();
    let mut worst = 0.0f64;
    let mut at = String::new();
    let mut steps = 0;
    let mut keys: Vec<&(String, u64, usize)> = runs.atoms.keys().filter(|k| k.2 < 5).collect();
    keys.sort();
    for key in keys {
        let (mode, seed, i) = key;
        let mut rates = ReactionRates::zero();
        for n in &names {
            rates.set(n, MicroRate { gamma: runs.gamma[key][n], fission: runs.fission[key][n], n2n: 0.0 });
        }
        let n0: Vec<f64> = names.iter().map(|n| runs.atoms[key][n]).collect();
        let ours = cram48(&chain.build_matrix(&rates), &n0, 86_400.0);
        let next = &runs.atoms[&(mode.clone(), *seed, i + 1)];
        let big = ours.iter().cloned().fold(0.0, f64::max);
        for (j, n) in names.iter().enumerate() {
            let r = next[n];
            if r.abs() < 1e-12 * big {
                continue;
            }
            let rel = ((ours[j] - r) / r).abs();
            if rel > worst {
                worst = rel;
                at = format!("{mode} seed {seed} step {i}->{} {n}: ours {:e}, OpenMC {r:e}", i + 1, ours[j]);
            }
        }
        steps += 1;
    }
    println!("{steps} OpenMC steps replayed; worst {worst:.2e} ({at})");
    assert_eq!(steps, 2 * 16 * 5);
    assert!(worst <= 1e-10, "{worst}");
}

#[test]
fn port_against_openmc_within_statistics() {
    let mut runs = Runs::default();
    read(&outputs().join("pincell_depletion.tsv"), &mut runs);
    read(&outputs().join("pincell_depletion_port.tsv"), &mut runs);
    let seeds = |mode: &str| -> Vec<u64> {
        let mut s: Vec<u64> = runs.k.keys().filter(|k| k.0 == mode && k.2 == 0).map(|k| k.1).collect();
        s.sort();
        s
    };
    let (port, flux, direct) = (seeds("port"), seeds("flux"), seeds("direct"));
    assert_eq!((port.len(), flux.len(), direct.len()), (16, 16, 16));
    let mut worst_z = 0.0f64;
    let mut over3 = 0;
    println!("step  quantity       port                       OpenMC flux                z     OpenMC direct/flux - 1");
    for i in 0..=5 {
        let kp: Vec<f64> = port.iter().map(|s| runs.k[&("port".into(), *s, i)].0).collect();
        let kf: Vec<f64> = flux.iter().map(|s| runs.k[&("flux".into(), *s, i)].0).collect();
        let kd: Vec<f64> = direct.iter().map(|s| runs.k[&("direct".into(), *s, i)].0).collect();
        let ((mp, sp), (mf, sf), (md, _)) = (mean_sem(&kp), mean_sem(&kf), mean_sem(&kd));
        let z = (mp - mf) / (sp * sp + sf * sf).sqrt();
        println!("{i}     k          {mp:.5} +- {sp:.5}         {mf:.5} +- {sf:.5}      {z:+.2}   {:+.1} pcm", (md - mf) * 1e5);
        worst_z = worst_z.max(z.abs());
        over3 += usize::from(z.abs() > 3.0);
        if i == 0 {
            continue;
        }
        for n in ["U235", "U238", "I135", "Xe135", "Xe136", "Cs135", "Gd157"] {
            let get = |mode: &str, s: &[u64]| -> Vec<f64> { s.iter().map(|x| runs.atoms[&(mode.into(), *x, i)][n]).collect() };
            let ((mp, sp), (mf, sf), (md, _)) = (mean_sem(&get("port", &port)), mean_sem(&get("flux", &flux)), mean_sem(&get("direct", &direct)));
            let z = (mp - mf) / (sp * sp + sf * sf).sqrt();
            println!("{i}     {n:<6}  {mp:.6e} +- {sp:.1e}   {mf:.6e} +- {sf:.1e}   {z:+.2}   {:+.3} %", (md / mf - 1.0) * 100.0);
            worst_z = worst_z.max(z.abs());
            over3 += usize::from(z.abs() > 3.0);
        }
    }
    println!("worst |z| {worst_z:.2}; {over3} of 41 above 3");
    assert_eq!(over3, 0, "worst |z| {worst_z}");
}
