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
//! # Results, measured 2026-11-17/18 (all three predictions held)
//!
//! 1. Check 1: 160 OpenMC steps (2 modes x 16 seeds x 5) replayed, worst
//!    **4.1e-15** (CRAM-48 dense against SuperLU, as arm 1).
//! 2. Check 2: **0 of 41 above 3 sigma, worst |z| 1.62** (Xe-135, day 5).
//!    `k`: 1.16158 +- 0.00074 against 1.16222 +- 0.00046 at BOS (-0.7
//!    sigma), 1.14221 +- 0.00066 against 1.14245 +- 0.00077 at day 5 (-0.2
//!    sigma); U-235 at day 5 agrees within 0.1 sigma.
//!
//!    **RE-MEASURED 2026-11-18** with H in H2O bound per material
//!    (`assign_material_sab`, gh:#462; the example had bound it to the H-1
//!    nuclide, which `tests/no_unmarked_nuclide_level_sab_462.rs` flags).
//!    Only the water holds H-1, so the physics is the same, but the random
//!    stream is not, so all 16 seeds were re-run (prediction on gh:#1626
//!    first: 0 of 41 above 3 sigma, held). ~~0 of 41, worst 1.76 (Gd-157,
//!    step 3); `k` 1.16286 +- 0.00090 at BOS (+0.6 sigma), 1.14343 +-
//!    0.00110 at day 5 (+0.7 sigma)~~, the nuclide-level run. Day-5 `k`
//!    moved -122 pcm, -0.95 sigma on the two runs' combined error: slightly
//!    more than the old run's own SEM (0.00110), which was the bound
//!    predicted.
//!
//!    ~~First run: 0 of 41 above 3 sigma, worst 2.85, with U-238 low at every
//!    step (-1.9 to -2.8 sigma), recorded as a candidate systematic.~~
//!    **CORRECTED 2026-11-17:** that run had U-234 fission at zero
//!    (`reaction_from_ace` did not build U-234's redundant MT=18, found by
//!    `tests/collapse_rate_vs_libopenmc.rs`). With it fixed and all 16 seeds
//!    re-run, U-238 sits at -0.6 to -1.3 sigma: the signal was that defect,
//!    acting through the fission-power normalisation, not transport. The
//!    first run's output is not kept in the repository.
//! 3. OpenMC direct against flux (CCFE-709) at day 5: Xe-135 -0.19 %,
//!    Cs-135 -0.18 %, Gd-157 -0.13 %, I-135 -0.003 %, Xe-136 +0.014 %, U
//!    below 1e-5 %; all under the predicted 0.5 %. `k` within 166 pcm
//!    (k carries ~100 pcm noise per seed-mean here).
//!
//! Wall time, one host (Intel Xeon Processor, 4 logical cores, 4 threads
//! each, 15 GB RAM, Linux 6.18, CPU only): OpenMC 19 s per seed (5 steps +
//! final); the port 166-176 s on the nuclide-level run (seeds 3-16, host
//! not shared) and 174-181 s on the per-material re-run (seeds 9-16, host
//! not shared), about 9x. The re-run's seeds 1-8 (244-364 s) shared the
//! host with the nee_soon test suite and are not timings.

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

/// **Solver ablation** (maintainer direction 2026-11-17: the solvers are
/// PETIR's; measure how much choosing among them moves the result). The
/// replay of [`openmc_rates_through_the_port_integrator`] through each of
/// PETIR's matrix exponentials, against OpenMC's end-of-step atoms:
/// `Cram48` and `Cram16` (IPF form, ported into PETIR from OpenMC's
/// `cram.py`) and `cram16_pfd` (ONIX's partial-fraction CRAM-16, an
/// independent port). Same filter as check 1.
///
/// **Prediction, posted on gh:#1626 before the run:** `Cram48` unchanged
/// (4.1e-15); `Cram16` IPF at most 1e-10; ONIX PFD at most 1e-9 and above
/// IPF CRAM-16. Recorded, not gated: the assertions only require each
/// solver to return finite densities on every step.
///
/// **Results, measured 2026-11-17** (160 steps each; the bounds held):
///
/// | PETIR solver | worst relative to OpenMC | where |
/// |---|---|---|
/// | `Cram48` (IPF, OpenMC's default) | 4.07e-15 | direct seed 1, step 0->1, U-238 |
/// | `Cram16` (IPF) | 4.80e-14 | flux seed 6, step 0->1, Cs-135 |
/// | ONIX `cram16_pfd` | 4.97e-14 | direct seed 5, step 0->1, Cs-135 |
///
/// The choice of solver is worth at most ~5e-14 here, below anything the
/// transport comparison can see (its sigma is ~1e-3). The third part of the
/// prediction, that the partial-fraction form would be clearly worse than
/// IPF CRAM-16, did not show: the two agree to within 4 %.
#[test]
fn solver_ablation_on_openmc_rates() {
    let mut runs = Runs::default();
    read(&outputs().join("pincell_depletion.tsv"), &mut runs);
    let (chain, names) = chain();
    let mut keys: Vec<&(String, u64, usize)> = runs.atoms.keys().filter(|k| k.2 < 5).collect();
    keys.sort();
    let solvers: [(&str, fn(&outram_mc_libs::depletion::matrix::DepletionMatrix, &[f64], f64) -> Vec<f64>); 3] = [
        ("PETIR Cram48 (IPF)", |a, n, dt| petir::depletion::cram48(a, n, dt).unwrap()),
        ("PETIR Cram16 (IPF)", |a, n, dt| petir::depletion::cram16(a, n, dt).unwrap()),
        ("PETIR ONIX cram16_pfd", |a, n, dt| petir::depletion::cram16_pfd(a, n, dt).unwrap()),
    ];
    for (label, solve) in solvers {
        let (mut worst, mut at, mut steps) = (0.0f64, String::new(), 0);
        for key in &keys {
            let (mode, seed, i) = key;
            let mut rates = ReactionRates::zero();
            for n in &names {
                rates.set(n, MicroRate { gamma: runs.gamma[*key][n], fission: runs.fission[*key][n], n2n: 0.0 });
            }
            let n0: Vec<f64> = names.iter().map(|n| runs.atoms[*key][n]).collect();
            let ours = solve(&chain.build_matrix(&rates), &n0, 86_400.0);
            assert!(ours.iter().all(|x| x.is_finite()), "{label}: non-finite densities");
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
                    at = format!("{mode} seed {seed} step {i}->{} {n}", i + 1);
                }
            }
            steps += 1;
        }
        println!("{label:<24} {steps} steps, worst relative to OpenMC {worst:.2e} ({at})");
    }
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
